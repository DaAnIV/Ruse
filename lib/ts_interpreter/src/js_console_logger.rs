use boa_engine::{Finalize, JsResult, Trace};
use boa_runtime::ConsoleState;
use tracing::{debug, error, info, warn};

#[derive(Debug, Trace, Finalize)]
pub struct RuseJsConsoleLogger;

impl boa_runtime::Logger for RuseJsConsoleLogger {
    #[inline]
    fn log(
        &self,
        msg: String,
        state: &ConsoleState,
        _context: &mut boa_engine::Context,
    ) -> JsResult<()> {
        let indent = state.indent();
        debug!(target: "ruse::JSConsole", "{msg:>indent$}");
        Ok(())
    }

    #[inline]
    fn info(
        &self,
        msg: String,
        state: &ConsoleState,
        _context: &mut boa_engine::Context,
    ) -> JsResult<()> {
        let indent = state.indent();
        info!(target: "ruse::JSConsole", "{msg:>indent$}");
        Ok(())
    }

    #[inline]
    fn warn(
        &self,
        msg: String,
        state: &ConsoleState,
        _context: &mut boa_engine::Context,
    ) -> JsResult<()> {
        let indent = state.indent();
        warn!(target: "ruse::JSConsole", "{msg:>indent$}");
        Ok(())
    }

    #[inline]
    fn error(
        &self,
        msg: String,
        state: &ConsoleState,
        _context: &mut boa_engine::Context,
    ) -> JsResult<()> {
        let indent = state.indent();
        error!(target: "ruse::JSConsole", "{msg:>indent$}");
        Ok(())
    }
}
