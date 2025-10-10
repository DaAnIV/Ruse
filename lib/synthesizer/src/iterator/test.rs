#[cfg(feature = "test_helpers")]
#[allow(dead_code)]
pub mod iterator_test_helpers {
    use crate::{
        bank::*,
        context::{ContextArray, VariableMap},
        iterator::{
            bank_iterator::bank_iterator, multi_programs_map_product::ProgramChildrenIterator,
            seq_triple_iterator::seq_triple_iterator,
        },
        pure,
        synthesizer_context::{EmptySynthesizerData, SynthesizerWorkerContext},
        test::helpers::*,
    };
    use std::sync::Arc;

    use dashmap::DashMap;
    use itertools::Either;
    use ruse_object_graph::{
        location::{LocValue, Location},
        value::Value,
        vnum, GraphIdGenerator, Number,
    };

    use crate::{
        bank::ProgBank, opcode::ExprOpcode, prog::SubProgram,
        synthesizer_context::SynthesizerContext,
    };

    #[derive(Default, Debug, Clone)]
    pub struct TestBankConfig {}
    impl BankConfig for TestBankConfig {}

    #[derive(Default, Debug, Clone)]
    pub struct TestBank {
        bank: Vec<Vec<Arc<SubProgram>>>,
    }

    pub struct TestBankBuilder {
        batch: Vec<Arc<SubProgram>>,
    }

    impl BatchBuilder for TestBankBuilder {
        async fn add_program(&mut self, p: &Arc<SubProgram>) -> bool {
            self.batch.push(p.clone());
            true
        }
    }

    impl BankIterationBuilder for TestBankBuilder {
        type BatchBuilderType = TestBankBuilder;

        fn create_batch_builder(&self) -> Self::BatchBuilderType {
            TestBankBuilder { batch: vec![] }
        }

        async fn add_batch(&mut self, batch: Self::BatchBuilderType) {
            self.batch.extend(batch.batch);
        }

        async fn iter_programs(&self) -> impl Iterator<Item = &Arc<SubProgram>> {
            self.batch.iter()
        }
    }

    impl ProgBank for TestBank {
        type IterationBuilderType = TestBankBuilder;
        type BankConfigType = TestBankConfig;

        async fn new_with_config(_config: Self::BankConfigType) -> Self {
            TestBank { bank: vec![] }
        }

        async fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
            self.bank.iter().any(|iteration| {
                iteration.iter().any(|existing_program| {
                    existing_program.out_value().eq(
                        existing_program.post_ctx(),
                        p.out_value(),
                        p.post_ctx(),
                    )
                })
            })
        }

        fn iteration_count(&self) -> usize {
            self.bank.len()
        }

        fn total_number_of_programs(&self) -> usize {
            self.bank.iter().map(|iteration| iteration.len()).sum()
        }

        async fn number_of_programs(
            &self,
            iteration: usize,
            output_type: &ruse_object_graph::ValueType,
        ) -> usize {
            if iteration >= self.bank.len() {
                return 0;
            }
            self.bank[iteration]
                .iter()
                .filter(|p| p.out_type() == output_type)
                .count()
        }

        async fn iter_programs<'a, 'b>(
            &'a self,
            iteration: usize,
            output_type: &'b ruse_object_graph::ValueType,
        ) -> impl Iterator<Item = &'a Arc<SubProgram>> + Send + 'a {
            let iter: Either<_, _> = if iteration >= self.bank.len() {
                Either::Left(std::iter::empty::<&Arc<SubProgram>>())
            } else {
                let output_type_clone = output_type.clone();
                Either::Right(
                    self.bank[iteration]
                        .iter()
                        .filter(move |p| p.out_type() == &output_type_clone),
                )
            };
            iter.into_iter()
        }

        fn create_iteration_builder(&self) -> Self::IterationBuilderType {
            TestBankBuilder { batch: vec![] }
        }

        async fn end_iteration(&mut self, iteration: Self::IterationBuilderType) {
            self.bank.push(iteration.batch);
        }
    }

    pub async fn run_gatherer(
        bank: &TestBank,
        op: &Arc<dyn ExprOpcode>,
        skip: Option<usize>,
        take: Option<usize>,
    ) -> Vec<Vec<Arc<SubProgram>>> {
        let all_children = Arc::new(DashMap::<usize, Vec<Arc<SubProgram>>>::default());
        let mut children_iterator = bank_iterator(bank, op.arg_types()).await;
        if let Some(skip_count) = skip {
            children_iterator.skip(skip_count).await;
        }
        if let Some(take_count) = take {
            children_iterator.take(take_count);
        }

        let mut iter = seq_triple_iterator(children_iterator);
        while let Some(triple) = iter.next().await {
            all_children.insert(all_children.len(), triple.children);
        }

        all_children.iter().map(|x| x.value().clone()).collect()
    }

    async fn create_bank(num_iterations: usize) -> TestBank {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        for i in 0..num_iterations {
            add_iteration(&mut bank, 3 + i, &syn_ctx).await;
        }
        bank
    }

    pub fn get_prog_for_bank(
        value: Value,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Arc<SubProgram> {
        let init_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: value,
            }),
        });

        let mut p = SubProgram::with_opcode(
            init_op,
            syn_ctx.start_context.clone(),
            syn_ctx.start_context.clone(),
        );
        Arc::get_mut(&mut p).unwrap().evaluate(syn_ctx, worker_ctx);
        p
    }

    pub fn print_all_children(all_children: &[Vec<Arc<SubProgram>>]) {
        for c in all_children {
            let values: Vec<String> = c
                .iter()
                .map(|x| {
                    let num = x.out_value()[0].val().number_value().unwrap().0 as u64;
                    format!("{:x}", num)
                })
                .collect();
            println!("{:?}", values);
        }
    }

    pub async fn add_iteration(bank: &mut impl ProgBank, n: usize, syn_ctx: &SynthesizerContext) {
        let mut worker_ctx = SynthesizerWorkerContext {
            index: 0,
            data: Box::new(EmptySynthesizerData {}),
        };
        let iteration = bank.iteration_count();
        let mut iteration_builder = bank.create_iteration_builder();
        let mut batch_builder = iteration_builder.create_batch_builder();
        for i in 0..n {
            let value = Number::from(iteration << 32 | i);
            let p = get_prog_for_bank(vnum!(value), syn_ctx, &mut worker_ctx);
            batch_builder.add_program(&p).await;
        }
        iteration_builder.add_batch(batch_builder).await;
        bank.end_iteration(iteration_builder).await;
    }
}

#[cfg(test)]
mod bank_iterator_tests {
    use crate::{
        context::VariableMap,
        iterator::{
            iterations_iterator::IterationsIterator,
            multi_programs_map_product::{multi_programs_map_product, ProgramChildrenIterator},
            seq_triple_iterator::seq_triple_iterator,
            test::iterator_test_helpers::*,
        },
        pure,
        test::helpers::*,
    };
    use std::sync::Arc;

    use itertools::Itertools;
    use ruse_object_graph::{
        location::{LocValue, Location},
        vnum, GraphIdGenerator, Number, ValueType,
    };

    use crate::{
        bank::ProgBank, context::ContextArray, opcode::ExprOpcode,
        synthesizer_context::SynthesizerContext,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn programs_map_multi_iter() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        add_iteration(&mut bank, 2, &syn_ctx).await;
        let mut iter = seq_triple_iterator(
            multi_programs_map_product(
                &bank,
                [(0, ValueType::Number), (0, ValueType::Number)].into_iter(),
            )
            .await,
        );
        while let Some(triple) = iter.next().await {
            println!(
                "{:#?}",
                triple
                    .children
                    .iter()
                    .map(|p| p.out_value()[0].val.number_value().unwrap())
                    .collect_vec()
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_iterations_iterator_one_iterations() {
        let mut iterator = IterationsIterator::new(1, 1);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0])));
        assert_eq!(iterator.next(), None);

        let mut iterator = IterationsIterator::new(1, 2);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 0])));
        assert_eq!(iterator.next(), None);

        let mut iterator = IterationsIterator::new(1, 3);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 0, 0])));
        assert_eq!(iterator.next(), None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_iterations_iterator_two_iterations() {
        let mut iterator = IterationsIterator::new(2, 1);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1])));
        assert_eq!(iterator.next(), None);

        let mut iterator = IterationsIterator::new(2, 2);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 1])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 0])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 1])));
        assert_eq!(iterator.next(), None);

        let mut iterator = IterationsIterator::new(2, 3);
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 0, 1])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 1, 0])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([0, 1, 1])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 0, 0])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 0, 1])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 1, 0])));
        assert_eq!(iterator.next(), Some(Vec::<usize>::from([1, 1, 1])));
        assert_eq!(iterator.next(), None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn programs_map_multi_iter_skip() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        for i in 0..(3 * 4) {
            let mut children_iter = multi_programs_map_product(
                &bank,
                [(0, ValueType::Number), (1, ValueType::Number)].into_iter(),
            )
            .await;
            if i > 0 {
                children_iter.skip(i).await;
            }

            let mut count = 0;
            let mut triple_iter = seq_triple_iterator(children_iter);
            while let Some(triple) = triple_iter.next().await {
                count += 1;
                println!(
                    "{}",
                    triple
                        .children
                        .iter()
                        .map(|p| {
                            let num = p.out_value()[0].val.number_value().unwrap().0 as usize;
                            format!("{}:{}", num >> 32, num & 0xFFFFFFFF)
                        })
                        .join(", ")
                );
            }
            assert_eq!(count, 3 * 4 - i);
            println!("total: {}", count);
            println!("");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn programs_map_multi_iter_take() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        for i in 0..(3 * 4) {
            let mut children_iter = multi_programs_map_product(
                &bank,
                [(0, ValueType::Number), (1, ValueType::Number)].into_iter(),
            )
            .await;
            if i > 0 {
                children_iter.take(3 * 4 - i)
            }

            let mut count = 0;
            let mut triple_iter = seq_triple_iterator(children_iter);
            while let Some(triple) = triple_iter.next().await {
                count += 1;
                println!(
                    "{}",
                    triple
                        .children
                        .iter()
                        .map(|p| {
                            let num = p.out_value()[0].val.number_value().unwrap().0 as usize;
                            format!("{}:{}", num >> 32, num & 0xFFFFFFFF)
                        })
                        .join(", ")
                );
            }
            assert_eq!(count, 3 * 4 - i);
            println!("total: {}", count);
            println!("");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn programs_map_multi_iter_skip_take() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        let mut children_iter = multi_programs_map_product(
            &bank,
            [(0, ValueType::Number), (1, ValueType::Number)].into_iter(),
        )
        .await;
        children_iter.skip(5).await;
        children_iter.take(3);

        let mut count = 0;
        let mut triple_iter = seq_triple_iterator(children_iter);
        while let Some(triple) = triple_iter.next().await {
            count += 1;
            println!(
                "{}",
                triple
                    .children
                    .iter()
                    .map(|p| {
                        let num = p.out_value()[0].val.number_value().unwrap().0 as usize;
                        format!("{}:{}", num >> 32, num & 0xFFFFFFFF)
                    })
                    .join(", ")
            );
        }
        assert_eq!(count, 3);
        println!("total: {}", count);
        println!("");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn one_iteration_one_program() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 1, &syn_ctx).await;

        let all_children = run_gatherer(&bank, &bin_op, None, None).await;
        assert_eq!(all_children.len(), 1);
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_iterations() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        assert!(bank.iteration_count() == 2);

        let all_children = run_gatherer(&bank, &bin_op, None, None).await;
        assert_eq!(all_children.len(), 5usize.pow(2) - 2usize.pow(2));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        let all_children = run_gatherer(&bank, &bin_op, None, None).await;
        assert_eq!(all_children.len(), 9usize.pow(2) - 5usize.pow(2));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_trinary() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let tri_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        let all_children = run_gatherer(&bank, &tri_op, None, None).await;
        assert_eq!(all_children.len(), 9usize.pow(3) - 5usize.pow(3));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary_skip() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        for skip in 0..(9usize.pow(2) - 5usize.pow(2)) {
            let skip_opt = if skip > 0 { Some(skip) } else { None };
            let all_children = run_gatherer(&bank, &bin_op, skip_opt, None).await;
            println!("{}", all_children.len());
            assert_eq!(all_children.len(), 9usize.pow(2) - 5usize.pow(2) - skip);
            for c in &all_children {
                c.iter().take(5).for_each(|x| {
                    let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                    print!("{}:{}, ", num >> 32, num & 0xFFFFFFFF);
                });
                println!("");
            }
            println!("");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary_take() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        for take in 0..=(9usize.pow(2) - 5usize.pow(2)) {
            let all_children = run_gatherer(&bank, &bin_op, None, Some(take)).await;
            println!("{}", all_children.len());
            assert_eq!(all_children.len(), take);
            for c in &all_children {
                c.iter().take(5).for_each(|x| {
                    let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                    print!("{}:{}, ", num >> 32, num & 0xFFFFFFFF);
                });
                println!("");
            }
            println!("");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary_skip_take() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        let all_children = run_gatherer(&bank, &bin_op, Some(5), Some(3)).await;
        println!("{}", all_children.len());
        assert_eq!(all_children.len(), 3);
        for c in &all_children {
            c.iter().take(5).for_each(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                print!("{}:{}, ", num >> 32, num & 0xFFFFFFFF);
            });
            println!("");
        }
        println!("");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary_split() {
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx =
            SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
        let mut bank = TestBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: pure!(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx).await;
        add_iteration(&mut bank, 3, &syn_ctx).await;
        add_iteration(&mut bank, 4, &syn_ctx).await;

        let total_size = 9usize.pow(2) - 5usize.pow(2);
        let split_count = 10;

        let all_children = run_gatherer(&bank, &bin_op, None, None).await;
        let mut all_children_split = Vec::with_capacity(split_count);
        for i in 0..split_count {
            let skip = (total_size / split_count) * i;
            let take = if i == split_count - 1 {
                usize::MAX
            } else {
                total_size / split_count
            };
            let part = run_gatherer(&bank, &bin_op, Some(skip), Some(take)).await;
            all_children_split.push(part);
        }

        assert_eq!(
            all_children_split.iter().fold(0, |acc, x| acc + x.len()),
            total_size
        );
        assert!(all_children
            .iter()
            .all(|x| { all_children_split.iter().any(|part| part.contains(x)) }));
    }
}
