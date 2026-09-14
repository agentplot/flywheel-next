//! The order confirmation email: the total the customer saw at checkout, in
//! the currency they paid in, never a field left unset.

use crate::money::Money;

/// What the confirmation says the order came to. A total is always the
/// charged amount, so an order with no charge yet has no confirmation to send.
pub fn total_line(charged: Option<Money>) -> Option<String> {
    let total = charged?;
    Some(format!("Order total: {}", total.formatted()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_total_is_the_charged_amount() {
        let line = total_line(Some(Money::usd_cents(4_250))).expect("a charged order has a total");
        assert_eq!(line, "Order total: $42.50");
    }

    #[test]
    fn an_uncharged_order_has_no_confirmation() {
        assert_eq!(total_line(None), None, "never `undefined`");
    }
}
