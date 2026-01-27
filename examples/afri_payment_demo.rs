use Bitmesh_backend::chains::stellar::{
    client::StellarClient,
    config::StellarConfig,
    payment_builder::{MemoType, PaymentBuilder},
};
use rust_decimal::Decimal;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = StellarConfig::from_env()?;
    let client = StellarClient::new(config.clone())?;

    let source_account = "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU";
    let destination = "GD2I2A7CGJTPXQYPX6J5RQBAVXVWX3LNZUQVLWZAXVLBW3RN6WLMLQHF";
    let afri_issuer = "GCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU";
    let amount = Decimal::from_str("10.50")?;

    let mut builder = PaymentBuilder::new(client, config)
        .with_source_account(source_account);

    builder
        .add_payment_op(destination, amount, "AFRI", afri_issuer)
        .await?;

    builder.add_memo(MemoType::Text, "Payment for goods")?;

    let estimated_fee = builder.estimate_fee().await?;
    println!("Estimated fee: {} stroops", estimated_fee);

    let secret_key = "SCCVPYFOHY7ZB7557JKENAX62LUAPLMGIWNZJAFV2MITK6T32V37KEJU";
    let signed_tx = builder.sign_tx(secret_key).await?;

    println!("Transaction signed successfully!");
    println!("Source: {}", signed_tx.source_account);
    println!("Sequence: {}", signed_tx.sequence_number);
    println!("Fee: {} stroops", signed_tx.fee);
    println!("Operations: {}", signed_tx.operations.len());

    Ok(())
}
