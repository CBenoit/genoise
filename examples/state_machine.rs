use genoise::local;
use genoise::GnState;

enum StateMachineResponse {
    Waiting,
    SendPayload(String),
}

async fn tls_client_sequence(
    mut co: local::StackCo<'_, StateMachineResponse, String>,
) -> Result<(), ()> {
    // Send ClientHello message
    let received = co
        .suspend(StateMachineResponse::SendPayload("ClientHello".to_owned()))
        .await;

    // Check received message
    if received != "ServerHello" {
        return Err(());
    }

    while co.suspend(StateMachineResponse::Waiting).await != "ServerHelloDone" {
        // Process initial negotiation messages from server
    }

    // Generate and exchange the client key
    co.suspend(StateMachineResponse::SendPayload(
        "ClientKeyExchange".to_owned(),
    ))
    .await;

    // Tell the server to change to encrypted mode
    co.suspend(StateMachineResponse::SendPayload(
        "ChangeCipherSpec".to_owned(),
    ))
    .await;

    // Tell the server that we are ready for secure data communication to begin
    let received = co
        .suspend(StateMachineResponse::SendPayload("Finished".to_owned()))
        .await;

    if received != "ChangeCipherSpec" {
        return Err(());
    }

    let received = co.suspend(StateMachineResponse::Waiting).await;

    if received != "Finished" {
        return Err(());
    }

    Ok(())
}

async fn tls_server_sequence(
    mut co: local::StackCo<'_, StateMachineResponse, String>,
) -> Result<(), ()> {
    let received = co.suspend(StateMachineResponse::Waiting).await;

    if received != "ClientHello" {
        return Err(());
    }

    co.suspend(StateMachineResponse::SendPayload("ServerHello".to_owned()))
        .await;

    co.suspend(StateMachineResponse::SendPayload("Certificate".to_owned()))
        .await;

    co.suspend(StateMachineResponse::SendPayload(
        "ServerKeyExchange".to_owned(),
    ))
    .await;

    let received = co
        .suspend(StateMachineResponse::SendPayload(
            "ServerHelloDone".to_owned(),
        ))
        .await;

    if received != "ClientKeyExchange" {
        return Err(());
    }

    while co.suspend(StateMachineResponse::Waiting).await != "Finished" {
        // Process initial negotiation messages from server
    }

    co.suspend(StateMachineResponse::SendPayload(
        "ChangeCipherSpec".to_owned(),
    ))
    .await;

    co.suspend(StateMachineResponse::SendPayload("Finished".to_owned()))
        .await;

    Ok(())
}

fn main() -> Result<(), ()> {
    local::let_gen!(client_sequence, co, { tls_client_sequence(co) });
    local::let_gen!(server_sequence, co, { tls_server_sequence(co) });

    let mut client_state = client_sequence.start();
    let _ = server_sequence.start();

    loop {
        let client_response = match client_state {
            GnState::Suspended(StateMachineResponse::Waiting) => String::new(),
            GnState::Suspended(StateMachineResponse::SendPayload(payload)) => payload,
            GnState::Completed(res) => {
                res?;
                break;
            }
        };

        if !client_response.is_empty() {
            println!("-> {client_response}");
        }

        let server_state = server_sequence.resume(client_response);

        let server_response = match server_state {
            GnState::Suspended(StateMachineResponse::Waiting) => String::new(),
            GnState::Suspended(StateMachineResponse::SendPayload(payload)) => payload,
            GnState::Completed(res) => {
                res?;
                break;
            }
        };

        if !server_response.is_empty() {
            println!("<- {server_response}");
        }

        client_state = client_sequence.resume(server_response);
    }

    println!("Done.");

    Ok(())
}
