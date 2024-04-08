use ruse_object_graph::Cache;
use ruse_synthesizer::{context::Context, synthesizer::{OpcodesList, Statistics, Synthesizer}};
use ruse_ts_interpreter::opcode::TsExprAst;


pub struct TsSynthesizer<const N: usize>(Synthesizer<TsExprAst, N>);

impl<const N: usize> TsSynthesizer<N> {
    pub fn with_context_and_opcodes(
        start_context: [Context; N],
        opcodes: OpcodesList<TsExprAst>,
        cache: &mut Cache
    ) -> Self {
        Self {
            0: Synthesizer::with_context_and_opcodes(start_context, opcodes, cache)
        }    
    }

    #[inline]
    pub fn synthesize_for_size(&mut self, ctx: &[Context; N], n: usize, cache: &mut Cache) {
        self.0.synthesize_for_size(ctx, n, cache)
    }  

    #[inline]
    pub fn statistics(&self) -> &Statistics {
        &self.0.statistics()
    }
}