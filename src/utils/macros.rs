#[macro_export]
macro_rules! must {
    ($val: expr) => {
        match $val {
            Ok(x) => x,
            Err(e) => std::panic::panic_any($crate::errstr!(e))
        }
    };
    ($val: expr, $($hint:tt)*) => {
        match $val {
            Ok(x) => x,
            Err(e) => std::panic::panic_any(format!("{}: {}",
                format!($($hint)*),  $crate::errstr!(e)
            )),
        }
    };
}

#[macro_export]
macro_rules! errstr {
    ($e: ident) => {{
        // enable error trait
        use std::error::Error;
        let s = $e.to_string().to_lowercase();
        if let Some(e) = $e.source() {
            let ss = e.to_string().to_lowercase();
            format!("{}, caused by: {}", s, ss)
        } else {
            s
        }
    }};
}
