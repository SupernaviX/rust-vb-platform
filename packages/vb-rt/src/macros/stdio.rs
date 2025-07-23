/// Prints to standard output.
/// Note that the Virtual Boy has no standard output,
/// and this will only have an effect in the Lemur emulator.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let mut writer = $crate::stdio::OutWriter;
        writer.write(format_args!($($arg)*));
    };
}

/// Prints to standard output with a newline at the end.
/// Note that the Virtual Boy has no standard output,
/// and this will only have an effect in the Lemur emulator.
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
