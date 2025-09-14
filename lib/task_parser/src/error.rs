trait SnythesisInnerTaskError: std::fmt::Debug + Sized {
    fn line(&self) -> u32;
    fn path(&self) -> &str;

    fn message(&self) -> String;
}

macro_rules! impl_inner_task_error {
    ($ty:ty) => {
        impl std::error::Error for $ty {}
        
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "[{}:{}] {}", self.path(), self.line(), self.message())
            }
        }
        impl SnythesisInnerTaskError for $ty {
            fn line(&self) -> u32 {
                self.line
            }
            fn path(&self) -> &str {
                &self.path
            }
            fn message(&self) -> String {
                self.format_message()
            }
        }
    }
}


#[derive(Debug)]
pub struct TodoError {
    pub path: String,
    pub line: u32,
    pub to_implement: &'static str,
}

impl TodoError {
    fn format_message(&self) -> String {
        format!("{} not yet implemented", self.to_implement)
    }
}
#[derive(Debug)]
pub struct ParseError {
    pub path: String,
    pub line: u32,
    pub value: String,
    pub error: Box<dyn std::error::Error>,
}

impl ParseError {
    fn format_message(&self) -> String {
        format!("Failed to parse {}, Error: {}", self.value, self.error)
    }
}

#[derive(Debug)]
pub struct VerifyError {
    pub path: String,
    pub line: u32,
    pub msg: String,
}


impl VerifyError {
    fn format_message(&self) -> String {
        format!("Verification error: {}", self.msg)
    }
}

#[derive(Debug)]
pub struct SkipError {
    pub path: String,
    pub line: u32,
    pub reason: String,
}

impl SkipError {
    fn format_message(&self) -> String {
        format!("Skipped, reason: {}", self.reason)
    }
}

#[derive(Debug)]
pub struct SynthesisTaskIoError {
    pub path: String,
    pub line: u32,
    pub error: std::io::Error,
}

impl SynthesisTaskIoError {
    fn format_message(&self) -> String {
        format!("IO error: {}", self.error)
    }
}

impl_inner_task_error!(TodoError);
impl_inner_task_error!(ParseError);
impl_inner_task_error!(VerifyError);
impl_inner_task_error!(SkipError);
impl_inner_task_error!(SynthesisTaskIoError);

#[derive(Debug)]
pub enum SnythesisTaskError {
    IO(SynthesisTaskIoError),
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
            line: std::line!(),
            path: std::file!().to_string(),
            value: $val.to_string(),
            error: $e.into(),
        })
    };
}

#[macro_export]
macro_rules! verify_err {
    ($msg:expr) => {
        $crate::error::SnythesisTaskError::Verify($crate::error::VerifyError {
            line: std::line!(),
            path: std::file!().to_string(),            
            msg: $msg.to_owned(),
        })
    };
    ($($arg:tt)*) => {
        $crate::error::SnythesisTaskError::Verify($crate::error::VerifyError {
            line: std::line!(),
            path: std::file!().to_string(),
            msg: format!($($arg)*),
        })
    };
}

#[macro_export]
macro_rules! skip_err {
    ($reason:expr) => {
        $crate::error::SnythesisTaskError::Skip($crate::error::SkipError {
            line: std::line!(),
            path: std::file!().to_string(),
            reason: $reason.to_owned(),
        })
    };
    ($($arg:tt)*) => {
        $crate::error::SnythesisTaskError::Skip($crate::error::SkipError {
            line: std::line!(),
            path: std::file!().to_string(),
            reason: format!($($arg)*),
        })
    };
}

#[macro_export]
macro_rules! io_err {
    ($inner_error:expr) => {
        $crate::error::SnythesisTaskError::IO($crate::error::SynthesisTaskIoError {
            line: std::line!(),
            path: std::file!().to_string(),
            error: $inner_error.into(),
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

pub type SynthesisTaskResult<T> = Result<T, SnythesisTaskError>;
