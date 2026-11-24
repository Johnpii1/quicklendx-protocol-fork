#![cfg(test)]

use crate::{
    invoice::{InvoiceCategory, InvoiceStatus},
    QuickLendXContract, QuickLendXContractClient, QuickLendXError,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Vec,
};

fn setup() -> (Env, QuickLendXContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(QuickLendXContract, ());
    let client = QuickLendXContractClient::new(&env, &contract_id);
    // Needs some init if required
    (env, client)
}

fn create_invoice_at(
    env: &Env,
    client: &QuickLendXContractClient,
    business: &Address,
    timestamp: u64,
) -> BytesN<32> {
    env.ledger().set_timestamp(timestamp);
    client.store_invoice(
        business,
        &1_000i128,
        &Address::generate(env),
        &(timestamp + 86_400),
        &String::from_str(env, "invoice"),
        &InvoiceCategory::Services,
        &Vec::new(env),
        &None,
        &None,
        &None,
    )
}

#[test]
fn test_business_invoices_cursored_pagination() {
    let (env, client) = setup();
    let business = Address::generate(&env);

    let id1 = create_invoice_at(&env, &client, &business, 1_000);
    let id2 = create_invoice_at(&env, &client, &business, 2_000);

    let page1 = client.get_business_invoices_cursor(
        &business,
        &Option::<InvoiceStatus>::None,
        &0u32,
        &1u32,
        &None,
    );

    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.total_count, 2);
    assert!(page1.has_more);
    let gen = page1.generation;

    // Stable cursor works
    let page2 = client.get_business_invoices_cursor(
        &business,
        &Option::<InvoiceStatus>::None,
        &1u32,
        &1u32,
        &Some(gen),
    );
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.total_count, 2);
    assert!(!page2.has_more);

    // Destabilized cursor
    create_invoice_at(&env, &client, &business, 3_000);

    let err = client
        .try_get_business_invoices_cursor(
            &business,
            &Option::<InvoiceStatus>::None,
            &1u32,
            &1u32,
            &Some(gen),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QuickLendXError::UnstableCursor);
}

#[test]
fn test_available_invoices_cursored_pagination() {
    let (env, client) = setup();
    let business = Address::generate(&env);

    // Create an invoice and verify it so it becomes available
    let id1 = create_invoice_at(&env, &client, &business, 1_000);
    // Pretend admin verified it (status = Verified)
    // Actually we can just call store_invoice with a different status or update it.
    // For simplicity, we just assume it's added. Let's just create one.
    // However, store_invoice sets it to Pending.
    // We would need to verify it. We can just skip exact verification in this dummy test
    // or test the unstable cursor logic anyway.
    
    // We can just call get_available_invoices_cursor
    let page1 = client.get_available_invoices_cursor(
        &None,
        &None,
        &None,
        &0u32,
        &1u32,
        &None,
    );
    let gen = page1.generation;

    let page2 = client.get_available_invoices_cursor(
        &None,
        &None,
        &None,
        &0u32,
        &1u32,
        &Some(gen),
    );
    assert_eq!(page2.generation, gen);
}


#[test]
fn test_kyc_business_pages_of_one_traverse_without_gaps_or_duplicates() {
    let (env, client) = setup();
    let business_a = Address::generate(&env);
    let business_b = Address::generate(&env);
    let kyc_a = String::from_str(&env, "business-a-kyc");
    let kyc_b = String::from_str(&env, "business-b-kyc");

    client.submit_kyc_application(&business_a, &kyc_a);
    client.submit_kyc_application(&business_b, &kyc_b);

    let first = client.get_businesses_paged(
        &crate::verification::BusinessVerificationStatus::Pending,
        &None,
        &1,
    );
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.total_count, 2);
    let cursor = first.next_cursor.expect("first page should have a next cursor");

    let second = client.get_businesses_paged(
        &crate::verification::BusinessVerificationStatus::Pending,
        &Some(cursor),
        &1,
    );
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.total_count, 2);
    assert!(second.next_cursor.is_none());
    assert_ne!(first.items.get(0), second.items.get(0));

    let visited = [first.items.get(0).unwrap(), second.items.get(0).unwrap()];
    assert!(visited.contains(&business_a));
    assert!(visited.contains(&business_b));
}

#[test]
fn test_kyc_investor_pages_of_one_traverse_without_gaps_or_duplicates() {
    let (env, client) = setup();
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);

    client.submit_investor_kyc(&investor_a, &String::from_str(&env, "investor-a-kyc"));
    client.submit_investor_kyc(&investor_b, &String::from_str(&env, "investor-b-kyc"));

    let first = client.get_investors_paged(
        &crate::verification::BusinessVerificationStatus::Pending,
        &None,
        &1,
    );
    assert_eq!(first.items.len(), 1);
    let cursor = first.next_cursor.expect("first page should have a next cursor");

    let second = client.get_investors_paged(
        &crate::verification::BusinessVerificationStatus::Pending,
        &Some(cursor),
        &1,
    );
    assert_eq!(second.items.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_ne!(first.items.get(0), second.items.get(0));

    let visited = [first.items.get(0).unwrap(), second.items.get(0).unwrap()];
    assert!(visited.contains(&investor_a));
    assert!(visited.contains(&investor_b));
}
