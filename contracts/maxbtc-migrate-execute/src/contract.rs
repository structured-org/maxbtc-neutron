use crate::{
    error::ContractResult,
    msg::{ExecuteMsg, MigrateMsg, QueryMsg},
};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response};
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};

use crate::msg::InstantiateMsg;

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> ContractResult<Response<NeutronMsg>> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(_deps: Deps<NeutronQuery>, _env: Env, _msg: QueryMsg) -> ContractResult<Binary> {
    unimplemented!();
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    _deps: DepsMut<NeutronQuery>,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> ContractResult<Response<NeutronMsg>> {
    unimplemented!();
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn migrate(
    _deps: DepsMut<NeutronQuery>,
    _env: Env,
    msg: MigrateMsg,
) -> ContractResult<Response<NeutronMsg>> {
    Ok(Response::new().add_messages(msg.msgs))
}
