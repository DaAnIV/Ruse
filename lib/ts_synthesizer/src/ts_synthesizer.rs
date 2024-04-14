use ruse_object_graph::Cache;
use ruse_synthesizer::{bank::ContextArray, synthesizer::{OpcodesList, Statistics, Synthesizer}};
use ruse_ts_interpreter::opcode::TsExprAst;


pub struct TsSynthesizer<const N: usize>(Synthesizer<TsExprAst, N>);

impl<const N: usize> TsSynthesizer<N> {
    pub fn with_context_and_opcodes(
        start_context: ContextArray<N>,
        opcodes: OpcodesList<TsExprAst>,
        cache: &Cache
    ) -> Self {
        Self {
            0: Synthesizer::with_context_and_opcodes(start_context, opcodes, cache)
        }    
    }

    #[inline]
    pub fn synthesize_for_size(&mut self, ctx: &ContextArray<N>, n: usize, cache: &Cache) {
        self.0.synthesize_for_size(ctx, n, cache)
    }  

    #[inline]
    pub fn statistics(&self) -> &Statistics {
        &self.0.statistics()
    }
}