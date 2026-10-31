use serde::{Deserialize, Serialize};

macro_rules! id {
    ($($name:ident),*) => {$(
        /// V1's `AutoNumber` key for this table.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub i32);
    )*};
}

id!(
    CustomerId, ContactId, RepId, ProductId, QuoteId, OrderId,
    PaymentId
);
