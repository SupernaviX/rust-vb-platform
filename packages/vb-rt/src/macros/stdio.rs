#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let mut writer = $crate::stdio::OutWriter;
        writer.write(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::println!("");
    };
    ($($arg:tt)*) => {
        let mut writer = $crate::stdio::OutWriter;
        let _ = core::fmt::Write::write_fmt(&mut writer, format_args!($($arg)*));
        writer.write_nl();
    };
}
