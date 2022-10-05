use {
    anchor_lang::{prelude::Pubkey, solana_program::sysvar, InstructionData},
    anchor_spl::{associated_token, token},
    clockwork_sdk::{
        queue_program::{self, accounts::Trigger},
        Client, ClientResult, SplToken,
    },
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        native_token::LAMPORTS_PER_SOL,
        signature::Keypair,
        signer::Signer,
        system_program,
    },
};

fn main() -> ClientResult<()> {
    // Create Client
    let payer = Keypair::new();
    #[cfg(feature = "devnet")]
    let client = Client::new(payer, "https://api.devnet.solana.com".into());
    #[cfg(not(feature = "devnet"))]
    let client = Client::new(payer, "http://localhost:8899".into());
    client.airdrop(&client.payer_pubkey(), 2 * LAMPORTS_PER_SOL)?;

    // create token mint
    let mint = client
        .create_token_mint(&client.payer_pubkey(), 9)
        .unwrap()
        .pubkey();

    // Derive PDAs
    let recipient = Keypair::new().pubkey();
    let payment = payments_program::state::Payment::pubkey(client.payer_pubkey(), recipient, mint);
    let payment_queue =
        clockwork_sdk::queue_program::accounts::Queue::pubkey(payment, "payment".into());

    // airdrop to payment queue
    client.airdrop(&payment_queue, LAMPORTS_PER_SOL)?;

    // Create ATAs
    let sender_token_account =
        client.create_associated_token_account(&client.payer(), &client.payer_pubkey(), &mint)?;
    let recipient_token_account =
        client.create_associated_token_account(&client.payer(), &recipient, &mint)?;

    // get escrow associated token address
    let escrow = anchor_spl::associated_token::get_associated_token_address(&payment, &mint);

    // mint to sender's associated token account
    client.mint_to(
        &client.payer(),
        &mint,
        &sender_token_account,
        LAMPORTS_PER_SOL,
        9,
    )?;

    create_payment_with_top_up(
        &client,
        recipient,
        sender_token_account,
        recipient_token_account,
        payment_queue,
        payment,
        mint,
        escrow,
    )?;

    update_payment(&client, recipient, payment_queue, payment, mint)?;

    Ok(())
}

fn create_payment_with_top_up(
    client: &Client,
    recipient: Pubkey,
    sender_token_account: Pubkey,
    recipient_token_account: Pubkey,
    payment_queue: Pubkey,
    payment: Pubkey,
    mint: Pubkey,
    escrow: Pubkey,
) -> ClientResult<()> {
    // create ix
    let create_payment_ix = Instruction {
        program_id: payments_program::ID,
        accounts: vec![
            AccountMeta::new_readonly(associated_token::ID, false),
            AccountMeta::new_readonly(queue_program::ID, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(payment, false),
            AccountMeta::new(payment_queue, false),
            AccountMeta::new_readonly(recipient, false),
            AccountMeta::new_readonly(recipient_token_account, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(client.payer_pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        data: payments_program::instruction::CreatePayment {
            disbursement_amount: 10000,
            schedule: "*/15 * * * * * *".into(),
        }
        .data(),
    };

    let top_up_payment_ix = Instruction {
        program_id: payments_program::ID,
        accounts: vec![
            AccountMeta::new_readonly(associated_token::ID, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new(payment, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(recipient, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(client.payer_pubkey(), true),
            AccountMeta::new(sender_token_account, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        data: payments_program::instruction::TopUpPayment {
            amount: LAMPORTS_PER_SOL,
        }
        .data(),
    };

    print_tx_sig(
        client,
        &[create_payment_ix, top_up_payment_ix],
        "create_payment_with_top_up".to_string(),
    )?;

    println!(
        "queue: https://explorer.solana.com/address/{}?cluster=custom",
        payment_queue
    );

    Ok(())
}

fn update_payment(
    client: &Client,
    recipient: Pubkey,
    queue: Pubkey,
    payment: Pubkey,
    mint: Pubkey,
) -> ClientResult<()> {
    let update_queue_ix = Instruction {
        program_id: payments_program::ID,
        accounts: vec![
            AccountMeta::new_readonly(queue_program::ID, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(payment, false),
            AccountMeta::new(queue, false),
            AccountMeta::new_readonly(recipient, false),
            AccountMeta::new(client.payer_pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: payments_program::instruction::UpdatePayment {
            disbursement_amount: Some(100000),
            schedule: Some(Trigger::Cron {
                schedule: "*/20 * * * * * *".to_string(),
                skippable: true,
            }),
        }
        .data(),
    };
    print_tx_sig(client, &[update_queue_ix], "update_queue".to_string())?;
    Ok(())
}

fn print_tx_sig(client: &Client, ix: &[Instruction], label: String) -> ClientResult<()> {
    match client.send_and_confirm(ix, &[client.payer()]) {
        Ok(sig) => println!(
            "{} tx: ✅ https://explorer.solana.com/tx/{}?cluster=custom",
            label, sig
        ),
        Err(err) => println!("{} tx: ❌ {:#?}", label, err),
    }

    Ok(())
}
