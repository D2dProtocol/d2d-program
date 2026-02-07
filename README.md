# D2D Protocol - Solana Program

A decentralized deployment platform enabling developers to deploy Solana programs on mainnet at minimal cost through a staking/lending treasury model. D2D Protocol connects stakers (lenders), developers, and platform admin in a sustainable on-chain economic system.

**Program ID:** `HDxYgZcTu6snVtCEozCUkhwmmUngWEsYuNKJsvgpyL5k`

## Architecture Overview

```mermaid
graph TB
    subgraph Actors
        Staker["Staker (Lender)"]
        Dev["Developer"]
        Admin["Admin / Guardian"]
    end

    subgraph OnChain["D2D Solana Program"]
        TP["TreasuryPool PDA"]
        RP["RewardPool PDA"]
        PP["PlatformPool PDA"]
        LS["BackerDeposit PDA<br/>(per staker)"]
        DR["DeployRequest PDA<br/>(per deployment)"]
        MP["ManagedProgram PDA<br/>(per program)"]
        DE["DeveloperEscrow PDA<br/>(per developer)"]
        WQ["WithdrawalQueueEntry PDA<br/>(per queue position)"]
        PW["PendingWithdrawal PDA<br/>(admin timelock)"]
    end

    subgraph ExternalPrograms["Deployed Programs"]
        P1["Developer Program A"]
        P2["Developer Program B"]
    end

    Staker -->|stake_sol / unstake_sol| TP
    Staker -->|claim_rewards| RP
    Staker -->|queue_withdrawal| WQ

    Dev -->|request_deployment_funds| DR
    Dev -->|pay_subscription| RP
    Dev -->|proxy_upgrade_program| MP
    Dev -->|deposit_escrow_sol| DE

    Admin -->|fund_temporary_wallet| TP
    Admin -->|confirm_deployment| DR
    Admin -->|process_withdrawal_queue| WQ
    Admin -->|transfer_authority_to_pda| MP
    Admin -->|reclaim_program_rent| MP

    MP -->|"PDA authority (upgrade/close)"| P1
    MP -->|"PDA authority (upgrade/close)"| P2

    TP --- RP
    TP --- PP
```

## Economic Model

```mermaid
flowchart LR
    subgraph Inflow["SOL Inflow"]
        S1["Staker Deposits"]
        S2["Developer Service Fees"]
        S3["Monthly Subscriptions"]
        S4["Rent Recovery"]
    end

    subgraph Treasury["TreasuryPool"]
        LB["liquid_balance"]
        RPS["reward_per_share"]
        TB["total_borrowed"]
    end

    subgraph Pools["Fee Pools"]
        RP2["RewardPool<br/>(1% reward fee)"]
        PP2["PlatformPool<br/>(0.1% platform fee)"]
        PUR["pending_undistributed<br/>_rewards"]
    end

    subgraph Outflow["SOL Outflow"]
        O1["Deployment Funding"]
        O2["Staker Withdrawals"]
        O3["Reward Claims"]
        O4["Admin Withdrawals"]
    end

    S1 -->|"deposit - fees"| LB
    S1 -->|"1% fee"| RP2
    S1 -->|"0.1% fee"| PP2
    S2 --> RP2
    S3 --> RP2
    S4 -->|"debt repayment"| LB
    S4 -->|"excess"| RP2

    LB -->|"max 80% utilization"| O1
    LB --> O2
    RP2 --> O3
    RP2 -->|"first-depositor protection"| PUR
    PUR -->|"gradual distribution"| RPS

    PP2 --> O4
```

## Deployment Lifecycle

```mermaid
stateDiagram-v2
    [*] --> PendingDeployment: request_deployment_funds<br/>or create_deploy_request

    PendingDeployment --> Active: confirm_deployment_success<br/>(admin)
    PendingDeployment --> Failed: confirm_deployment_failure<br/>(admin, full refund)

    Active --> SubscriptionExpired: subscription expires
    Active --> Cancelled: close_program_and_refund

    SubscriptionExpired --> Active: pay_subscription<br/>or auto_renew
    SubscriptionExpired --> InGracePeriod: start_grace_period<br/>(admin)

    InGracePeriod --> Active: pay_subscription<br/>or auto_renew
    InGracePeriod --> Closed: close_expired_program<br/>(grace expired)

    Failed --> [*]
    Cancelled --> [*]
    Closed --> [*]: rent recovered<br/>debt repaid

    note right of Active
        Developer can upgrade
        via proxy_upgrade_program
    end note

    note right of InGracePeriod
        3/5/7 days based on
        subscription history
    end note
```

## PDA Authority Model

```mermaid
sequenceDiagram
    participant Dev as Developer
    participant D2D as D2D Program
    participant PDA as Authority PDA
    participant BPF as BPF Loader
    participant Prog as Developer's Program

    Note over Dev,Prog: Deploy Phase
    Dev->>D2D: request_deployment_funds()
    D2D->>D2D: Admin deploys program off-chain
    D2D->>PDA: transfer_authority_to_pda()
    PDA-->>Prog: PDA becomes upgrade authority

    Note over Dev,Prog: Upgrade Phase (Trustless)
    Dev->>BPF: Upload buffer (standard)
    Dev->>D2D: proxy_upgrade_program(buffer)
    D2D->>D2D: Verify: developer owns program<br/>+ subscription active
    D2D->>PDA: Sign upgrade CPI
    PDA->>BPF: BPFLoaderUpgradeable::upgrade()
    BPF->>Prog: Program upgraded

    Note over Dev,Prog: Rent Reclaim (Expired)
    D2D->>PDA: reclaim_program_rent()
    PDA->>BPF: Close program data
    BPF-->>D2D: SOL returned to treasury
    D2D->>D2D: Debt repaid, excess to rewards
```

## Staker Reward System

```mermaid
flowchart TB
    subgraph RewardSources["Reward Sources"]
        SF["Service Fees"]
        MF["Monthly Subscriptions"]
        DF["Deposit Fees (1%)"]
    end

    subgraph Distribution["Distribution Mechanism"]
        RPS["reward_per_share<br/>(accumulator)"]
        PUR2["pending_undistributed<br/>_rewards"]
        DW["Duration Weight<br/>(amount x time)"]
    end

    subgraph StakerRewards["Per-Staker Calculation"]
        BR["Base Rewards<br/>= deposited * reward_per_share<br/>- reward_debt"]
        DB["Duration Bonus<br/>= pending * (my_weight / total_weight)"]
        TR["Total = Base + Duration Bonus"]
    end

    SF --> RPS
    MF --> RPS
    DF --> RPS
    DF -->|"first depositor<br/>protection"| PUR2

    RPS --> BR
    PUR2 --> DB
    DW --> DB
    BR --> TR
    DB --> TR
```

## Withdrawal Queue

```mermaid
sequenceDiagram
    participant Staker
    participant D2D as D2D Program
    participant Queue as WithdrawalQueue
    participant Treasury as TreasuryPool

    Note over Staker,Treasury: Insufficient Liquidity
    Staker->>D2D: unstake_sol(amount)
    D2D-->>Staker: Error: InsufficientLiquidBalance

    Staker->>D2D: queue_withdrawal(amount)
    D2D->>Queue: Create entry at position N
    D2D->>Treasury: queued_withdrawal_amount += amount

    Note over Staker,Treasury: Liquidity Restored (rent recovery)
    Treasury->>Treasury: reclaim_program_rent()<br/>liquid_balance increases

    D2D->>Queue: process_withdrawal_queue(N)
    Queue->>Queue: Partial or full fulfillment
    D2D->>Staker: Transfer SOL
    D2D->>Treasury: Update totals

    Note over Staker,Treasury: Optional: Cancel
    Staker->>D2D: cancel_queued_withdrawal()
    D2D->>Queue: Mark as cancelled
    D2D->>Treasury: queued_withdrawal_amount -= amount
```

## Security Architecture

```mermaid
graph TB
    subgraph AdminSecurity["Admin Security Layer"]
        TL["Timelock Withdrawals<br/>(1h - 7d delay)"]
        GV["Guardian Veto<br/>(block suspicious tx)"]
        DL["Daily Withdrawal Limits"]
        EP["Emergency Pause"]
    end

    subgraph EconomicSafety["Economic Safety"]
        UL["80% Utilization Limit<br/>(20% always liquid)"]
        DT["Debt Tracking<br/>(per deployment)"]
        FDP["First-Depositor<br/>Arbitrage Protection"]
        RHU["Fee Rounding<br/>(round-half-up)"]
    end

    subgraph AccessControl["Access Control"]
        ADMIN["Admin: pool mgmt,<br/>deployments, config"]
        GUARD["Guardian: pause,<br/>veto withdrawals"]
        DEV2["Developer: own programs,<br/>escrow, subscriptions"]
        STKR["Staker: own deposits,<br/>rewards, queue"]
    end

    subgraph OnChainChecks["On-Chain Validations"]
        PDA2["PDA-based authority<br/>(no private keys)"]
        CO["checked_* arithmetic<br/>(overflow protection)"]
        PP3["Pause checks on<br/>all instructions"]
        SV["Subscription validation<br/>for upgrades"]
    end

    AdminSecurity --> OnChainChecks
    EconomicSafety --> OnChainChecks
    AccessControl --> OnChainChecks
```

## State Accounts

| Account | PDA Seeds | Purpose |
|---------|-----------|---------|
| **TreasuryPool** | `["treasury_pool"]` | Central pool: deposits, rewards, debt tracking, withdrawal queue, dynamic APY |
| **BackerDeposit** | `["lender_stake", staker]` | Per-staker: deposited amount, reward debt, duration weight, queued withdrawal |
| **DeployRequest** | `["deploy_request", ...]` | Per-deployment: status, fees, subscription, grace period, debt tracking |
| **ManagedProgram** | `["managed_program", program_id]` | Per-program: developer, authority PDA, upgrade count |
| **DeveloperEscrow** | `["developer_escrow", developer]` | Per-developer: SOL/USDC/USDT balances for auto-renewal |
| **WithdrawalQueueEntry** | `["withdrawal_queue", position]` | Per-queue-entry: staker, amount, partial fulfillment tracking |
| **PendingWithdrawal** | `["pending_withdrawal", ...]` | Admin timelock: amount, destination, execute_after, vetoed |
| **UserDeployStats** | `["user_stats", user]` | Per-user: deployment count, rate limiting |

### Sub-PDAs (Token Pools)

| PDA | Seeds | Purpose |
|-----|-------|---------|
| **RewardPool** | `["reward_pool"]` | Holds SOL for staker rewards |
| **PlatformPool** | `["platform_pool"]` | Holds SOL for platform revenue |
| **Authority PDA** | `["program_authority", program_id]` | Upgrade authority for managed programs |

## Instructions

### Initialization
| Instruction | Signer | Description |
|-------------|--------|-------------|
| `initialize` | Admin | Initialize treasury pool with APY and dev wallet |
| `reinitialize_treasury_pool` | Admin | Reinitialize with new parameters |
| `migrate_treasury_pool` | Admin | Migrate state for schema upgrades |

### Staker (Lender) Operations
| Instruction | Signer | Description |
|-------------|--------|-------------|
| `stake_sol` | Staker | Deposit SOL into treasury (1% reward fee + 0.1% platform fee) |
| `unstake_sol` | Staker | Withdraw SOL (if liquid balance sufficient) |
| `emergency_unstake` | Staker | Emergency withdrawal with reward settlement |
| `claim_rewards` | Staker | Claim base rewards + duration bonus |
| `queue_withdrawal` | Staker | Queue withdrawal when liquidity insufficient |
| `cancel_queued_withdrawal` | Staker | Cancel a queued withdrawal |

### Developer Operations
| Instruction | Signer | Description |
|-------------|--------|-------------|
| `request_deployment_funds` | Developer | Request deployment with service fee + subscription |
| `pay_subscription` | Developer | Pay monthly subscription (extends validity) |
| `proxy_upgrade_program` | Developer | Upgrade program via PDA proxy (trustless) |
| `initialize_escrow` | Developer | Create escrow account for auto-renewal |
| `deposit_escrow_sol` | Developer | Deposit SOL into escrow |
| `withdraw_escrow_sol` | Developer | Withdraw SOL from escrow |
| `toggle_auto_renew` | Developer | Enable/disable auto-renewal |
| `set_preferred_token` | Developer | Set preferred token (SOL/USDC/USDT) |

### Admin Operations
| Instruction | Signer | Description |
|-------------|--------|-------------|
| `create_deploy_request` | Admin | Create deployment request on behalf of developer |
| `fund_temporary_wallet` | Admin | Fund temp wallet for deployment (records debt) |
| `confirm_deployment` | Admin | Confirm deployment success/failure |
| `transfer_authority_to_pda` | Admin | Transfer program authority to D2D PDA |
| `reclaim_program_rent` | Admin | Reclaim rent from expired programs (repays debt) |
| `close_program_and_refund` | Admin | Close program and refund developer |
| `process_withdrawal_queue` | Admin | Fulfill queued withdrawals when liquidity available |
| `distribute_pending_rewards` | Admin | Gradually distribute pending rewards to stakers |
| `auto_renew_subscription` | Admin | Trigger auto-renewal from developer escrow |
| `start_grace_period` | Admin | Start grace period for expired subscription |
| `close_expired_program` | Admin | Close program after grace period expires |
| `force_rebalance` | Admin | Sync treasury balances |
| `sync_liquid_balance` | Admin | Sync liquid_balance with actual lamports |
| `force_reset_deployment` | Admin | Force reset a stuck deployment |
| `credit_fee_to_pool` | Admin | Credit fees to reward/platform pools |
| `emergency_pause` | Admin | Toggle emergency pause |

### Security Operations
| Instruction | Signer | Description |
|-------------|--------|-------------|
| `set_guardian` | Admin | Set guardian address |
| `set_timelock_duration` | Admin | Set timelock duration (1h-7d) |
| `set_daily_limit` | Admin | Set daily withdrawal limit |
| `initiate_withdrawal` | Admin | Initiate timelocked withdrawal |
| `execute_withdrawal` | Admin | Execute after timelock expires |
| `cancel_withdrawal` | Admin | Cancel pending withdrawal |
| `guardian_pause` | Guardian | Emergency pause by guardian |
| `guardian_veto` | Guardian | Veto a pending withdrawal |

## Key Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `REWARD_FEE_BPS` | 100 (1%) | Fee on deposits directed to reward pool |
| `PLATFORM_FEE_BPS` | 10 (0.1%) | Fee on deposits directed to platform pool |
| `PRECISION` | 1e12 | Reward-per-share precision multiplier |
| `MAX_UTILIZATION_BPS` | 8000 (80%) | Max pool utilization for deployments |
| `DEFAULT_BASE_APY_BPS` | 500 (5%) | Default base APY |
| `DEFAULT_MAX_APY_MULTIPLIER` | 30000 (3x) | Max APY multiplier at high utilization |
| `DEFAULT_TARGET_UTILIZATION` | 6000 (60%) | Target utilization for APY curve |
| `DEFAULT_TIMELOCK` | 86400s (24h) | Default admin withdrawal timelock |
| `MAX_EXTENSION_MONTHS` | 120 (10y) | Maximum subscription extension |

## Project Structure

```
programs/d2d-program-sol/src/
├── lib.rs                              # Program entry point, instruction dispatch
├── errors.rs                           # Error codes (40+ categorized errors)
├── events.rs                           # On-chain events (30+ event types)
├── states/
│   ├── treasury_pool.rs                # Central treasury with debt, queue, APY
│   ├── lender_stake.rs                 # Per-staker deposit & reward tracking
│   ├── deploy_request.rs              # Deployment lifecycle & subscription
│   ├── managed_program.rs             # PDA authority proxy for programs
│   ├── developer_escrow.rs            # Auto-renewal escrow (SOL/USDC/USDT)
│   ├── withdrawal_queue.rs            # Staker withdrawal queue entries
│   ├── pending_withdrawal.rs          # Admin timelocked withdrawals
│   └── user_deploy_stats.rs           # User deployment statistics
├── instructions/
│   ├── initialize.rs                   # Treasury initialization
│   ├── request_deployment_funds.rs    # Developer deployment request
│   ├── lender/
│   │   ├── stake_sol.rs               # Stake with first-depositor protection
│   │   ├── unstake_sol.rs             # Unstake with queue check
│   │   ├── claim_rewards.rs           # Claim base + duration bonus
│   │   ├── emergency_unstake.rs       # Emergency withdrawal
│   │   ├── queue_withdrawal.rs        # Queue when illiquid
│   │   └── cancel_queued_withdrawal.rs
│   ├── developer/
│   │   ├── pay_subscription.rs        # Monthly subscription payment
│   │   ├── proxy_upgrade_program.rs   # Trustless upgrade via PDA
│   │   ├── initialize_escrow.rs       # Create escrow account
│   │   ├── deposit_escrow_sol.rs      # Fund escrow
│   │   ├── withdraw_escrow_sol.rs     # Withdraw from escrow
│   │   ├── toggle_auto_renew.rs       # Toggle auto-renewal
│   │   └── set_preferred_token.rs     # Set payment token preference
│   └── admin/
│       ├── fund_temporary_wallet.rs   # Fund deployment (debt tracking)
│       ├── confirm_deployment.rs      # Confirm success/failure
│       ├── transfer_authority_to_pda.rs # Transfer authority to PDA
│       ├── reclaim_program_rent.rs    # Reclaim rent (debt repayment)
│       ├── process_withdrawal_queue.rs # Fulfill queued withdrawals
│       ├── distribute_pending_rewards.rs # Gradual reward distribution
│       ├── auto_renew_subscription.rs # Trigger auto-renewal
│       ├── start_grace_period.rs      # Start grace period
│       ├── close_expired_program.rs   # Close after grace
│       ├── close_program_and_refund.rs
│       ├── create_deploy_request.rs
│       ├── credit_fee_to_pool.rs
│       ├── admin_withdraw.rs
│       ├── admin_withdraw_reward_pool.rs
│       ├── close_treasury_pool.rs
│       ├── reinitialize_treasury_pool.rs
│       ├── migrate_treasury_pool.rs
│       ├── sync_liquid_balance.rs
│       ├── force_rebalance.rs
│       ├── force_reset_deployment.rs
│       ├── emergency_pause.rs
│       ├── set_guardian.rs
│       ├── guardian_pause.rs
│       ├── set_timelock_duration.rs
│       ├── set_daily_limit.rs
│       ├── initiate_withdrawal.rs
│       ├── execute_withdrawal.rs
│       ├── cancel_withdrawal.rs
│       └── guardian_veto.rs
```

## Build & Deploy

```bash
# Prerequisites
# - Solana CLI v2.1+
# - Anchor v0.31.1+ (CLI v0.32.1)
# - Rust toolchain

# Build
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Run tests
anchor test
```

## License

MIT
