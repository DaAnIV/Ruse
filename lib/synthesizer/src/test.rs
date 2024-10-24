#[cfg(test)]
mod helpers {
    use ruse_object_graph::value::ValueType;

    use crate::{
        context::{Context, SynthesizerContext},
        location::LocValue,
        opcode::{EvalResult, ExprAst, ExprOpcode},
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
    pub struct TestOpcode {
        pub arg_types: Vec<ValueType>,
        pub returns: EvalResult,
    }

    impl ExprOpcode for TestOpcode {
        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(&self, _: &[&LocValue], _: &mut Context, _: &SynthesizerContext) -> EvalResult {
            self.returns.clone()
        }

        fn to_ast(&self, _: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {})
        }
    }
}


#[cfg(test)]
mod bank_iterator_tests {
    use crate::{
        bank::ProgramsMap, bank_iterator::bank_iterator, context::GraphIdGenerator, multi_programs_map_product::multi_programs_map_product, test::helpers::*
    };
    use std::sync::Arc;

    use dashmap::DashMap;
    use itertools::Itertools;
    use ruse_object_graph::{
        value::{Value, ValueType},
        vnum, Cache, Number,
    };

    use crate::{
        bank::{ProgBank, TypeMap},
        context::{ContextArray, SynthesizerContext},
        location::{LocValue, Location},
        opcode::{EvalResult, ExprOpcode},
        prog::SubProgram,
    };

    async fn run_gatherer(
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
    ) -> Vec<Vec<Arc<SubProgram>>> {
        let all_children = Arc::new(DashMap::<usize, Vec<Arc<SubProgram>>>::default());
        for triplet in bank_iterator(bank, op.arg_types()) {
            all_children.insert(all_children.len(), triplet.children);
        }
        
        all_children.iter().map(|x| x.value().clone()).collect()
    }

    fn get_prog_for_bank(value: Value, syn_ctx: &SynthesizerContext) -> Arc<SubProgram> {
        let init_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: value,
            }),
        });

        let mut p = SubProgram::with_opcode(
            init_op,
            syn_ctx.start_context.clone(),
            syn_ctx.start_context.clone(),
        );
        Arc::get_mut(&mut p).unwrap().evaluate(syn_ctx);
        p
    }

    #[allow(dead_code)]
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

    fn create_programs_map(n: usize, syn_ctx: &SynthesizerContext) -> ProgramsMap {
        let map = ProgramsMap::default();
        for i in 0..n {
            let value = Number::from(i);
            let p = get_prog_for_bank(vnum!(value), syn_ctx);
            map.0.insert(p.clone().into(), p);
        }
        map
    }

    #[test]
    fn programs_map_ref_iterator() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(5, &syn_ctx);
        for p in map.iter() {
            println!("{}", p.out_value()[0].val.number_value().unwrap());
        }    
    }

    #[test]
    fn programs_map_ref_2_iterator() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(5, &syn_ctx);
        for (p1, p2) in map.iter().zip(map.iter()) {
            let n1 = p1.out_value()[0].val.number_value().unwrap();
            let n2 = p2.out_value()[0].val.number_value().unwrap();
            println!("({}, {})", n1, n2);
        }    
    }

    #[test]
    fn programs_map_multi_iter() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(2, &syn_ctx);
        let map_ptr = std::ptr::from_ref(&map);
        for triplet in multi_programs_map_product([map_ptr, map_ptr].into_iter()) {
            println!("{:#?}", triplet.children.iter().map(|p| p.out_value()[0].val.number_value().unwrap()).collect_vec());
        }    
    }

    #[tokio::test(flavor = "current_thread")]
    async fn one_iteration_one_program() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 1, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op).await;
        assert_eq!(all_children.len(), 1);
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_iterations() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op).await;
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
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op).await;
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
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let tri_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &tri_op).await;
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
}
