use std::str::FromStr;

use cosmwasm_std::{
    entry_point, to_json_binary, Deps, DepsMut, Env, MessageInfo, Response, SignedDecimal256,
    StdResult,
};

use crate::{
    msg::InstantiateMsg,
    state::{BinanceData, ConsensusOutcome, GetDataResponse, QueryMsg, SpotBalance},
};

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    Ok(Response::new())
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::GetData {} => Ok(to_json_binary(&GetDataResponse {
            last_published_data: Some(ConsensusOutcome {
                data: BinanceData {
                    spot_balances: vec![SpotBalance {
                        asset: "untrn".to_string(),
                        amount: SignedDecimal256::from_str("10.23").unwrap(),
                    }],
                },
            }),
        })?),
    }
}
