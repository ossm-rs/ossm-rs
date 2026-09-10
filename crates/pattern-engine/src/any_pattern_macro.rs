/// Generates the `AnyPattern` enum and all associated boilerplate from a
/// list of `Variant(Type)` pairs.
///
/// For each entry this produces:
/// - An `AnyPattern` enum variant wrapping the pattern type.
/// - A `From<Type>` impl for converting into `AnyPattern`.
/// - Delegation of `Pattern::run()` to the inner type.
/// - `BUILTIN_PATTERNS`: array of [`PatternMeta`] for each variant.
/// - `all_builtin()`: array of default-constructed `AnyPattern` instances.
///
/// ```ignore
/// define_patterns! {
///     Simple(Simple),
///     Deeper(Deeper),
/// }
/// ```
macro_rules! define_patterns {
    ($( $variant:ident($type:ident) ),+ $(,)?) => {
        pub enum AnyPattern {
            $( $variant($type), )+
        }

        impl AnyPattern {
            pub const BUILTIN_PATTERNS: [PatternMeta; define_patterns!(@count $($variant)+)] = [
                $( PatternMeta { name: $type::NAME, description: $type::DESCRIPTION }, )+
            ];

            pub fn all_builtin() -> [AnyPattern; define_patterns!(@count $($variant)+)] {
                [ $( AnyPattern::$variant($type), )+ ]
            }
        }

        impl Pattern for AnyPattern {
            const NAME: &'static str = "AnyPattern";
            const DESCRIPTION: &'static str = "Enum dispatch wrapper";

            async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), ossm::Cancelled> {
                match self { $( Self::$variant(p) => p.run(ctx).await, )+ }
            }
        }

        $( impl From<$type> for AnyPattern {
            fn from(p: $type) -> Self { Self::$variant(p) }
        } )+
    };

    (@count $($t:tt)+) => { 0 $( + define_patterns!(@one $t) )+ };
    (@one $t:tt) => { 1 };
}
