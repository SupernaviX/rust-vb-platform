macro_rules! bool_enum {
    (
        $(#[$enum_attr:meta])*
        $enum_vis:vis $name:ident($(#[$false_attr:meta])* $false:ident, $(#[$true_attr:meta])* $true:ident)
    ) => {
            $(#[$enum_attr])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #[repr(u8)]
            pub enum $name {
                $(#[$false_attr])*
                $false = 0,
                $(#[$true_attr])*
                $true = 1,
            }
            impl $name {
                const fn into_bits(self) -> u8 {
                    self as _
                }

                const fn from_bits(value: u8) -> Self {
                    match value {
                        0 => Self::$false,
                        _ => Self::$true,
                    }
                }
            }
    };
}

pub(crate) use bool_enum;
