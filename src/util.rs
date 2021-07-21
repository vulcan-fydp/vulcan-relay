use lazy_static::lazy_static;

#[macro_export]
macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

lazy_static! {
    pub static ref LOCAL_POOL: tokio_local::LocalPoolHandle = tokio_local::new_local_pool(2);
}
