use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use borsh::{BorshDeserialize, BorshSerialize};
use std::str::FromStr;

#[derive(BorshDeserialize, BorshSerialize, Debug, Default)]
struct RewardInfo {
    pub reward_state: u8,
    pub open_time: u64,
    pub end_time: u64,
    pub last_update_time: u64,
    pub emissions_per_second_x64: u128,
    pub reward_total_emissioned: u64,
    pub reward_claimed: u64,
    pub token_mint: Pubkey,
    pub token_vault: Pubkey,
    pub authority: Pubkey,
    pub reward_growth_global_x64: u128,
}

#[derive(BorshDeserialize, BorshSerialize, Debug, Default)]
struct PoolState {
    pub bump: [u8; 1],
    pub amm_config: Pubkey,
    pub owner: Pubkey,
    pub token_mint_0: Pubkey,
    pub token_mint_1: Pubkey,
    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,
    pub observation_key: Pubkey,
    pub mint_decimals_0: u8,
    pub mint_decimals_1: u8,
    pub tick_spacing: u16,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    pub padding3: u16,
    pub padding4: u16,
    pub fee_growth_global_0_x64: u128,
    pub fee_growth_global_1_x64: u128,
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,
    pub swap_in_amount_token_0: u128,
    pub swap_out_amount_token_1: u128,
    pub swap_in_amount_token_1: u128,
    pub swap_out_amount_token_0: u128,
    pub status: u8,
    pub padding: [u8; 7],
    pub reward_infos: [RewardInfo; 3],
    pub tick_array_bitmap: [u64; 16],
    pub total_fees_token_0: u64,
    pub total_fees_claimed_token_0: u64,
    pub total_fees_token_1: u64,
    pub total_fees_claimed_token_1: u64,
    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,
    pub open_time: u64,
    pub recent_epoch: u64,
    pub padding1: [u64; 24],
    pub padding2: [u64; 32],
    #[borsh_skip]
    pub _extra_data: Vec<u8>,
}

async fn fetch_pool_price(
    rpc_url: &str,
    pool_address: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = RpcClient::new(rpc_url.to_string());
    let pool_pubkey = Pubkey::from_str(pool_address)?;
    let account = client.get_account(&pool_pubkey)?;

    println!("账户数据长度: {}", account.data.len());
    println!("账户拥有者 (程序 ID): {}", account.owner);

    let raydium_clmm_program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK")?;
    if account.owner != raydium_clmm_program_id {
        return Err(format!(
            "账户 {} 不属于 Raydium CLMM 程序 ({})",
            pool_address, raydium_clmm_program_id
        )
        .into());
    }

    if account.data.len() < 8 {
        return Err("账户数据长度不足，无法解析鉴别器".into());
    }
    let pool_state = PoolState::try_from_slice(&account.data[8..])?;

    println!("token_mint_0: {}", pool_state.token_mint_0);
    println!("token_mint_1: {}", pool_state.token_mint_1);
    println!("token_vault_0 (SOL): {}", pool_state.token_vault_0);
    println!("token_vault_1 (USDC): {}", pool_state.token_vault_1);
    println!("sqrt_price_x64: {}", pool_state.sqrt_price_x64);
    println!("流动性: {}", pool_state.liquidity);
    println!("当前刻度: {}", pool_state.tick_current);
    println!("刻度间距: {}", pool_state.tick_spacing);
    println!("代币0小数位: {}", pool_state.mint_decimals_0);
    println!("代币1小数位: {}", pool_state.mint_decimals_1);
    println!("池状态: {}", pool_state.status);
    println!("池创建时间: {}", pool_state.open_time);
    println!("最近 epoch: {}", pool_state.recent_epoch);
    println!("tick_array_bitmap (前 2 个): {:?}", &pool_state.tick_array_bitmap[..2]);

    let sol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;
    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let is_token0_sol = pool_state.token_mint_0 == sol_mint && pool_state.token_mint_1 == usdc_mint;
    let is_token1_sol = pool_state.token_mint_0 == usdc_mint && pool_state.token_mint_1 == sol_mint;

    if !is_token0_sol && !is_token1_sol {
        return Err(format!(
            "池 {} 不是 SOL/USDC 交易对 (token_mint_0: {}, token_mint_1: {})",
            pool_address, pool_state.token_mint_0, pool_state.token_mint_1
        )
        .into());
    }

    if pool_state.liquidity == 0 {
        println!("警告: 池 {} 流动性为 0，可能不活跃", pool_address);
    }
    if pool_state.status != 0 {
        println!("警告: 池状态为 {}，可能被暂停或限制", pool_state.status);
    }
    if pool_state.recent_epoch < 400 {
        println!("警告: 最近 epoch 为 {}，池可能不活跃（当前 epoch 应为 450-500）", pool_state.recent_epoch);
    }

    let sqrt_price = pool_state.sqrt_price_x64 as f64 / 2f64.powi(64);
    let raw_price = sqrt_price * sqrt_price;
    let (adjusted_price, pair) = if is_token0_sol {
        (raw_price, "SOL/USDC")
    } else {
        (1.0 / raw_price, "USDC/SOL")
    };

    // let decimal_adjustment = 10f64.powi((pool_state.mint_decimals_1 as i32) - (pool_state.mint_decimals_0 as i32));
    // let display_price = adjusted_price * decimal_adjustment;
    let decimal_adjustment = 10f64.powi((pool_state.mint_decimals_0 as i32) - (pool_state.mint_decimals_1 as i32));
    let display_price = adjusted_price * decimal_adjustment;


    println!("池地址: {}", pool_address);
    println!("代币对: {}", pair);
    println!("当前价格: {:.4} USDC per SOL", display_price);
    if display_price < 10.0 {
        println!("警告: 价格异常低（{:.4} USDC per SOL），池可能不活跃或流动性分布偏离", display_price);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_url = "https://api.mainnet-beta.solana.com";
    let pool_addresses = vec![
        "8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj", // 池地址，可以是任意地址
        "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv", // 池地址
    ];

    for pool_address in pool_addresses {
        println!("\n正在检查池: {}", pool_address);
        match fetch_pool_price(rpc_url, pool_address).await {
            Ok(_) => println!("池 {} 检查完成", pool_address),
            Err(e) => eprintln!("池 {} 获取价格失败: {}", pool_address, e),
        }
    }

    Ok(())
}