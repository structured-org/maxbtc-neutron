use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG, DENOM, TOKEN_METADATA};
use cosmwasm_std::{
    entry_point, to_json_binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdResult, SubMsg,
};
use cw2::set_contract_version;
use cw_ownable::initialize_owner;
use maxbtc_base::msg::token::{create_set_denom_metadata_msg, get_full_denom};
use neutron_std::types::cosmos::base::v1beta1::Coin;
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgCreateDenom, MsgMint};

const CONTRACT_NAME: &str = concat!("crates.io:structured-maxbtc__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CREATE_DENOM_REPLY_ID: u64 = 1;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    let cfg = Config {
        allowed_denom: msg.allowed_denom.clone(),
    };
    CONFIG.save(deps.storage, &cfg)?;
    DENOM.save(deps.storage, &msg.subdenom)?;
    TOKEN_METADATA.save(deps.storage, &msg.token_metadata)?;

    let create_denom_submsg = SubMsg::reply_on_success(
        Into::<CosmosMsg>::into(MsgCreateDenom {
            sender: env.contract.address.to_string(),
            subdenom: msg.subdenom.clone(),
        }),
        CREATE_DENOM_REPLY_ID,
    );
    Ok(Response::new()
        .add_submessage(create_denom_submsg)
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("allowed_denom", msg.allowed_denom)
        .add_attribute("subdenom", msg.subdenom))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<cosmwasm_std::Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&cfg)?)
        }
        QueryMsg::Denom {} => Ok(to_json_binary(&DENOM.load(deps.storage)?)?),
        QueryMsg::Ownership {} => Ok(to_json_binary(&cw_ownable::get_ownership(deps.storage)?)?),
    }
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => {
            cw_ownable::update_ownership(deps.into_empty(), &env.block, &info.sender, action)?;
            Ok(Response::new().add_attribute("action", "update_ownership"))
        }
        ExecuteMsg::Wrap {} => execute_wrap(deps, env, info),
    }
}

fn execute_wrap(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let coin = cw_utils::one_coin(&info)?;
    let denom = DENOM.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    assert!(!config.allowed_denom.is_empty());
    assert_eq!(config.allowed_denom, coin.denom);

    let mint_msg = Into::<CosmosMsg>::into(MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom,
            amount: coin.amount.to_string(),
        }),
        mint_to_address: info.sender.to_string(),
    });
    Ok(Response::new().add_message(mint_msg))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        CREATE_DENOM_REPLY_ID => {
            let subdenom = DENOM.load(deps.storage)?;
            let full_denom = get_full_denom(env.contract.address.to_string(), subdenom);
            DENOM.save(deps.storage, &full_denom)?;

            let token_metadata = TOKEN_METADATA.load(deps.storage)?;
            TOKEN_METADATA.remove(deps.storage);

            let msg = create_set_denom_metadata_msg(
                env.contract.address.into_string(),
                full_denom.clone(),
                token_metadata.clone(),
            )?;

            Ok(Response::new()
                .add_message(msg)
                .add_attribute("action", "reply-set-token-metadata")
                .add_attribute("denom", full_denom)
                .add_attribute("exponent", token_metadata.exponent.to_string())
                .add_attribute("display", token_metadata.display)
                .add_attribute("name", token_metadata.name)
                .add_attribute("description", token_metadata.description)
                .add_attribute("symbol", token_metadata.symbol)
                .add_attribute("uri", token_metadata.uri.unwrap_or_default())
                .add_attribute("uri_hash", token_metadata.uri_hash.unwrap_or_default()))
        }
        id => Err(ContractError::UnknownReplyId { id }),
    }
}
