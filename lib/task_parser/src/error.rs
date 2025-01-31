#[derive(Debug)]
pub struct TodoError {
    pub to_implement: &'static str,
}

impl TodoError {
    pub fn new(to_implement: &'static str) -> Self {
        Self { to_implement }
    }
}

impl std::error::Error for TodoError {}

impl std::fmt::Display for TodoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} not yet implemented", self.to_implement)
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub value: String,
    pub error: Box<dyn std::error::Error>,
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse {}, Error: {}", self.value, self.error)
    }
}

#[derive(Debug)]
pub struct VerifyError {
    pub msg: String,
}

impl From<&str> for VerifyError {
    fn from(value: &str) -> Self {
        VerifyError {
            msg: value.to_owned(),
        }
    }
}

impl std::error::Error for VerifyError {}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Verification error: {}", self.msg)
    }
}

#[derive(Debug)]
pub struct SkipError {
    pub reason: String,
}

impl From<&str> for SkipError {
    fn from(reason: &str) -> Self {
        SkipError {
            reason: reason.to_owned(),
        }
    }
}

impl std::error::Error for SkipError {}

impl std::fmt::Display for SkipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Skipped, reason: {}", self.reason)
    }
}

#[derive(Debug)]
pub enum SnythesisTaskError {
    IO(std::io::Error),
    Verify(VerifyError),
    Eval(boa_engine::JsError),
    Parse(ParseError),
    Skip(SkipError),
}

impl std::error::Error for SnythesisTaskError {}

#[macro_export]
macro_rules! parse_err {
    ($val:expr, $e:expr) => {
        $crate::error::SnythesisTaskError::Parse($crate::error::ParseError {
            value: $val.to_string(),
            error: $e.into(),
        })
    };
}

#[macro_export]
macro_rules! verify_err {
    ($msg:expr) => {
        $crate::error::SnythesisTaskError::Verify($crate::error::VerifyError {
            msg: $msg.to_owned(),
        })
    };
}

#[macro_export]
macro_rules! skip_err {
    ($reason:expr) => {
        $crate::error::SnythesisTaskError::Skip($crate::error::SkipError {
            reason: $reason.to_owned(),
        })
    };
}

impl std::fmt::Display for SnythesisTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnythesisTaskError::IO(e) => write!(f, "{}", e),
            SnythesisTaskError::Verify(e) => write!(f, "{}", e),
            SnythesisTaskError::Eval(e) => write!(f, "{}", e),
            SnythesisTaskError::Parse(e) => write!(f, "{}", e),
            SnythesisTaskError::Skip(e) => write!(f, "{}", e),
        }
    }
}
