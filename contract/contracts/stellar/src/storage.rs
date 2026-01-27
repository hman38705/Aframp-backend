use soroban_sdk::{Env, Address, Map};

const ADMIN_KEY: &str = "ADMIN";
const BALANCES_KEY: &str = "BALANCES";

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(ADMIN_KEY, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(ADMIN_KEY).unwrap()
}

fn balances(env: &Env) -> Map<Address, i128> {
    env.storage()
        .instance()
        .get(BALANCES_KEY)
        .unwrap_or(Map::new(env))
}

pub fn get_balance(env: &Env, user: &Address) -> i128 {
    balances(env).get(user).unwrap_or(0)
}

pub fn set_balance(env: &Env, user: &Address, amount: i128) {
    let mut map = balances(env);
    map.set(user.clone(), amount);
    env.storage().instance().set(BALANCES_KEY, &map);
}
