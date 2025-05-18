#[macro_export]
macro_rules! main {
    ($body:ident) => {
        #[unsafe(no_mangle)]
        pub fn _vb_rt_main() {
            $body()
        }
    };
}
