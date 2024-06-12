#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dashmap::DashMap;
    use itertools::Itertools;
    use ruse_object_graph::{Cache, Number};

    use crate::{
        bank::{ProgBank, TypeMap},
        context::{Context, SynthesizerContext},
        context_array,
        opcode::{ExprAst, ExprOpcode},
        prog::SubProgram,
        value::{LocValue, Location, Value, ValueType},
        vnum,
        work_gatherer::WorkGather,
    };

    pub struct TestAst {}

    impl ExprAst for TestAst {
        fn to_string(&self) -> String {
            "".to_owned()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[derive(Debug)]
    struct TestOpcode {
        pub arg_types: Vec<ValueType>,
        pub returns: Option<LocValue>,
    }

    impl ExprOpcode for TestOpcode {
        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(
            &self,
            _: &[&LocValue],
            _: &mut Context,
            _: &SynthesizerContext,
        ) -> Option<LocValue> {
            self.returns.clone()
        }

        fn to_ast(&self, _: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
            Box::new(TestAst {})
        }
    }

    async fn run_gatherer(
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        chunk_size: usize,
    ) -> Vec<Vec<Arc<SubProgram>>> {
        let cancel_token = Default::default();
        let all_children = Arc::new(DashMap::<usize, Vec<Arc<SubProgram>>>::default());
        let all_children_clone = all_children.clone();
        let mut gatherer = WorkGather::new(
            Arc::new(
                move |_: Arc<dyn ExprOpcode>, children: Vec<Arc<SubProgram>>| {
                    all_children_clone.insert(all_children_clone.len(), children);
                    None
                },
            ),
            chunk_size,
            cancel_token,
        );
        gatherer.gather_work_for_next_iteration(bank, op).await;
        gatherer.wait_for_all_tasks().await;

        all_children.iter().map(|x| x.value().clone()).collect()
    }

    fn get_prog_for_bank(value: Value, syn_ctx: &SynthesizerContext) -> Arc<SubProgram> {
        let init_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: value,
            }),
        });

        let mut p = SubProgram::with_opcode_and_context(init_op, &syn_ctx.start_context);
        Arc::get_mut(&mut p).unwrap().evaluate(syn_ctx);
        p
    }

    fn print_all_children(all_children: &[Vec<Arc<SubProgram>>]) {
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

    fn add_iteration(bank: &mut ProgBank, n: usize, syn_ctx: &SynthesizerContext) {
        let iteration = bank.iteration_count();
        let type_map = Arc::new(TypeMap::default());
        for i in 0..n {
            let value = Number::from(iteration << 32 | i);
            let p = get_prog_for_bank(vnum!(value), syn_ctx);
            type_map.insert_program(p.clone());
        }
        bank.insert(type_map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn one_iteration_one_program() {
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(context_array![[]], cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 1, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1).await;
        assert_eq!(all_children.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_iterations() {
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(context_array![[]], cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1).await;
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
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(context_array![[]], cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1).await;
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
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(context_array![[]], cache);
        let mut bank = ProgBank::default();
        let tri_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number, ValueType::Number],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &tri_op, 1).await;
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
    async fn three_iterations_binary_big_chunk() {
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(context_array![[]], cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: Some(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 25).await;
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
}
