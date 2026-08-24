use soroban_sdk::{Address, Env, String, Vec};

use crate::events::Events;
use crate::pagination;
use crate::storage::Storage;
use crate::types::{ContractError, RegistryEntry, RegistryEntryType};

/// Add a contributor to the global registry index. Called on registration.
///
/// Does not itself check for a pre-existing entry: the `contributor_register`
/// entrypoint in lib.rs already performs an O(1) `Storage::get_contributor`
/// duplicate check before calling this function (and before the contributor
/// profile is written), so re-scanning the whole index here would be
/// redundant and O(n) per registration. Callers other than that entrypoint
/// must perform an equivalent duplicate check before calling this function.
pub fn register_contributor(
    env: &Env,
    address: &Address,
    name: &String,
) -> Result<(), ContractError> {
    let mut index = Storage::get_contributor_index(env);

    let entry = RegistryEntry {
        address: address.clone(),
        registered_at: env.ledger().timestamp(),
        is_active: true,
        entry_type: RegistryEntryType::Contributor,
    };

    index.push_back(entry);
    Storage::set_contributor_index(env, &index);

    Events::emit_contributor_registered(env, address.clone(), name.clone());

    Ok(())
}

/// Add an address to the approved reviewer allowlist. Admin only.
pub fn approve_reviewer(
    env: &Env,
    admin: &Address,
    reviewer: &Address,
) -> Result<(), ContractError> {
    require_global_admin(env, admin)?;

    let mut allowlist = Storage::get_reviewer_allowlist(env);

    if allowlist.contains(reviewer.clone()) {
        return Ok(());
    }

    allowlist.push_back(reviewer.clone());
    Storage::set_reviewer_allowlist(env, &allowlist);

    Events::emit_reviewer_approved(env, reviewer.clone(), admin.clone());

    Ok(())
}

/// Remove an address from the approved reviewer allowlist. Admin only.
pub fn revoke_reviewer(
    env: &Env,
    admin: &Address,
    reviewer: &Address,
) -> Result<(), ContractError> {
    require_global_admin(env, admin)?;

    let allowlist = Storage::get_reviewer_allowlist(env);
    let mut new_list: Vec<Address> = Vec::new(env);
    let mut found = false;

    for addr in allowlist.iter() {
        if addr == *reviewer {
            found = true;
        } else {
            new_list.push_back(addr);
        }
    }

    if !found {
        return Err(ContractError::InvalidInput);
    }

    Storage::set_reviewer_allowlist(env, &new_list);

    Events::emit_reviewer_revoked(env, reviewer.clone(), admin.clone());

    Ok(())
}

/// Check if an address is on the approved reviewer allowlist.
pub fn is_approved_reviewer(env: &Env, address: &Address) -> bool {
    let allowlist = Storage::get_reviewer_allowlist(env);
    allowlist.contains(address.clone())
}

/// Paginated list of all registered contributor addresses.
pub fn get_contributors_page(env: &Env, offset: u32, limit: u32) -> Vec<RegistryEntry> {
    let index = Storage::get_contributor_index(env);
    pagination::paginate(env, &index, offset, limit)
}

/// Total count of registered contributors.
pub fn contributor_count(env: &Env) -> u32 {
    Storage::get_contributor_index(env).len()
}

fn require_global_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
    let admin = Storage::get_global_admin(env).ok_or(ContractError::Unauthorized)?;
    if admin != *caller {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use soroban_sdk::{testutils::Address as _, Env, String};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        Storage::set_global_admin(&env, &admin);
        (env, admin)
    }

    #[test]
    fn test_register_contributor() {
        let (env, _) = setup();
        let addr = Address::generate(&env);
        let name = String::from_str(&env, "Alice");

        register_contributor(&env, &addr, &name).unwrap();

        assert_eq!(contributor_count(&env), 1);
        let page = get_contributors_page(&env, 0, 10);
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap().address, addr);
        assert_eq!(
            page.get(0).unwrap().entry_type,
            RegistryEntryType::Contributor
        );
        assert!(page.get(0).unwrap().is_active);
    }

    #[test]
    #[should_panic]
    fn test_register_contributor_duplicate() {
        let (env, _) = setup();
        let addr = Address::generate(&env);
        let name = String::from_str(&env, "Alice");

        register_contributor(&env, &addr, &name).unwrap();
        register_contributor(&env, &addr, &name).unwrap(); // duplicate
    }

    #[test]
    fn test_register_multiple_contributors() {
        let (env, _) = setup();
        let a1 = Address::generate(&env);
        let a2 = Address::generate(&env);
        let a3 = Address::generate(&env);

        register_contributor(&env, &a1, &String::from_str(&env, "A")).unwrap();
        register_contributor(&env, &a2, &String::from_str(&env, "B")).unwrap();
        register_contributor(&env, &a3, &String::from_str(&env, "C")).unwrap();

        assert_eq!(contributor_count(&env), 3);
    }

    #[test]
    fn test_approve_reviewer() {
        let (env, admin) = setup();
        let reviewer = Address::generate(&env);

        approve_reviewer(&env, &admin, &reviewer).unwrap();

        assert!(is_approved_reviewer(&env, &reviewer));
    }

    #[test]
    fn test_approve_reviewer_idempotent() {
        let (env, admin) = setup();
        let reviewer = Address::generate(&env);

        approve_reviewer(&env, &admin, &reviewer).unwrap();
        approve_reviewer(&env, &admin, &reviewer).unwrap(); // no-op

        assert!(is_approved_reviewer(&env, &reviewer));
    }

    #[test]
    #[should_panic]
    fn test_approve_reviewer_unauthorized() {
        let (env, _) = setup();
        let reviewer = Address::generate(&env);
        let other = Address::generate(&env);

        approve_reviewer(&env, &other, &reviewer).unwrap();
    }

    #[test]
    fn test_revoke_reviewer() {
        let (env, admin) = setup();
        let reviewer = Address::generate(&env);

        approve_reviewer(&env, &admin, &reviewer).unwrap();
        assert!(is_approved_reviewer(&env, &reviewer));

        revoke_reviewer(&env, &admin, &reviewer).unwrap();
        assert!(!is_approved_reviewer(&env, &reviewer));
    }

    #[test]
    #[should_panic]
    fn test_revoke_reviewer_not_found() {
        let (env, admin) = setup();
        let reviewer = Address::generate(&env);

        revoke_reviewer(&env, &admin, &reviewer).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_revoke_reviewer_unauthorized() {
        let (env, admin) = setup();
        let reviewer = Address::generate(&env);

        approve_reviewer(&env, &admin, &reviewer).unwrap();

        let other = Address::generate(&env);
        revoke_reviewer(&env, &other, &reviewer).unwrap();
    }

    #[test]
    fn test_is_approved_reviewer_returns_false_for_unknown() {
        let (env, _) = setup();
        let addr = Address::generate(&env);
        assert!(!is_approved_reviewer(&env, &addr));
    }

    #[test]
    fn test_pagination() {
        let (env, _) = setup();

        for i in 0..5 {
            let addr = Address::generate(&env);
            register_contributor(&env, &addr, &String::from_str(&env, "C")).unwrap();
        }

        assert_eq!(contributor_count(&env), 5);

        // Page 0: items 0-2
        let page0 = get_contributors_page(&env, 0, 3);
        assert_eq!(page0.len(), 3);

        // Page 1: items 3-4
        let page1 = get_contributors_page(&env, 3, 3);
        assert_eq!(page1.len(), 2);

        // Offset beyond count
        let empty = get_contributors_page(&env, 10, 3);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_contributor_count_empty() {
        let (env, _) = setup();
        assert_eq!(contributor_count(&env), 0);
    }
}
