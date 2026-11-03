// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Adapted from Chromium's components/streaming_client/streaming_websocket_client.cc, at tag
// 156.0.8078.47 (sync_relay.h says how).

#include "browser_core/sync_relay.h"

#include <limits>
#include <optional>
#include <utility>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "chrome/browser/net/system_network_context_manager.h"
#include "net/base/isolation_info.h"
#include "net/storage_access_api/status.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/constants.h"
#include "services/network/public/mojom/ip_address_space.mojom.h"
#include "services/network/public/mojom/network_context.mojom.h"
#include "url/origin.h"

namespace browser_core {

namespace {

// The server's frames are small: a pairing's are the largest, and a message's payload is at most
// 64 KiB.
constexpr size_t kMaxIncomingMessageSize = 1 << 20;

constexpr net::NetworkTrafficAnnotationTag kSyncRelay =
    net::DefineNetworkTrafficAnnotation("browser_core_sync_relay", R"(
      semantics {
        sender: "Sync"
        description:
          "Holds a connection to the server of the browser's account, on which it says another "
          "device changed what's synced, so this one fetches it at once rather than by polling, "
          "and passes pairing's messages between the account's devices."
        trigger: "The user signed in to an account in Settings › Account; held while signed in."
        data:
          "The account's access token. Pairing's messages: this device's name and system, a "
          "public key made for the pairing, and the account's secret sealed to the other device's "
          "key, which the server can't open."
        destination: OTHER
        destination_other: "The account's server: the build's own, or one the user named."
      }
      policy {
        cookies_allowed: NO
        setting: "Settings › Account: Sign Out."
        policy_exception_justification: "Only while the user is signed in to an account."
      })");

}  // namespace

SyncRelay::SyncRelay(const GURL& url, const std::string& token, Delegate* delegate)
    : delegate_(delegate),
      readable_watcher_(FROM_HERE, mojo::SimpleWatcher::ArmingPolicy::MANUAL) {
  network::mojom::NetworkContext* network_context =
      SystemNetworkContextManager::GetInstance()
          ? SystemNetworkContextManager::GetInstance()->GetContext()
          : nullptr;
  if (!network_context) {
    closed_ = true;
    return;
  }
  mojo::PendingRemote<network::mojom::WebSocketHandshakeClient> handshake_remote =
      handshake_receiver_.BindNewPipeAndPassRemote();
  // A network service that goes away mid-handshake answers nothing.
  handshake_receiver_.set_disconnect_handler(
      base::BindOnce(&SyncRelay::Close, base::Unretained(this), 0, 0));
  std::vector<network::mojom::HttpHeaderPtr> headers;
  headers.push_back(network::mojom::HttpHeader::New("Authorization", "Bearer " + token));
  const url::Origin origin = url::Origin::Create(url);
  network_context->CreateWebSocket(
      url, /*requested_protocols=*/{}, net::StorageAccessApiStatus::kNone,
      net::IsolationInfo::CreateForInternalRequest(origin), std::move(headers),
      network::OriginatingProcessId::browser(), origin,
      network::mojom::ClientSecurityState::New(), network::mojom::kWebSocketOptionBlockAllCookies,
      net::MutableNetworkTrafficAnnotationTag(kSyncRelay), std::move(handshake_remote),
      /*url_loader_network_observer=*/mojo::NullRemote(),
      /*auth_handler=*/mojo::NullRemote(),
      /*header_client=*/mojo::NullRemote(),
      /*throttling_profile_id=*/std::nullopt, network::GetNoOpNetworkRestrictionsId(),
      /*target_address_space=*/network::mojom::IPAddressSpace::kUnknown,
      /*target_network=*/std::nullopt);
}

SyncRelay::~SyncRelay() = default;

void SyncRelay::Send(const std::string& message) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  if (!open_ || message.size() > std::numeric_limits<uint32_t>::max()) {
    return;
  }
  websocket_->SendMessage(network::mojom::WebSocketMessageType::TEXT, message.size());
  if (writable_->WriteAllData(base::as_byte_span(message)) != MOJO_RESULT_OK) {
    Close(0, 0);
  }
}

void SyncRelay::Close(uint16_t code, int response_code) {
  if (closed_) {
    return;
  }
  closed_ = true;
  open_ = false;
  websocket_.reset();
  readable_watcher_.Cancel();
  readable_.reset();
  writable_.reset();
  client_receiver_.reset();
  handshake_receiver_.reset();
  pending_read_data_.clear();
  delegate_->OnRelayClosed(code, response_code);
}

void SyncRelay::OnOpeningHandshakeStarted(network::mojom::WebSocketHandshakeRequestPtr request) {}

void SyncRelay::OnFailure(const std::string& message, int net_error, int response_code) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  Close(0, response_code);
}

void SyncRelay::OnConnectionEstablished(
    mojo::PendingRemote<network::mojom::WebSocket> socket,
    mojo::PendingReceiver<network::mojom::WebSocketClient> client_receiver,
    network::mojom::WebSocketHandshakeResponsePtr response,
    mojo::ScopedDataPipeConsumerHandle readable,
    mojo::ScopedDataPipeProducerHandle writable) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  if (closed_) {
    return;
  }
  websocket_.Bind(std::move(socket));
  readable_ = std::move(readable);
  CHECK_EQ(readable_watcher_.Watch(readable_.get(), MOJO_HANDLE_SIGNAL_READABLE,
                                   MOJO_TRIGGER_CONDITION_SIGNALS_SATISFIED,
                                   base::BindRepeating(&SyncRelay::ReadFromDataPipe,
                                                       base::Unretained(this))),
           MOJO_RESULT_OK);
  writable_ = std::move(writable);
  client_receiver_.Bind(std::move(client_receiver));
  // The handshake's pipe closes now; the client's catches the network service going away.
  handshake_receiver_.set_disconnect_handler(base::DoNothing());
  client_receiver_.set_disconnect_handler(
      base::BindOnce(&SyncRelay::Close, base::Unretained(this), 0, 0));
  websocket_->StartReceiving();
  open_ = true;
  delegate_->OnRelayOpen();
}

void SyncRelay::OnDataFrame(bool finish,
                            network::mojom::WebSocketMessageType type,
                            uint64_t data_len) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  CHECK_EQ(pending_read_data_index_, pending_read_data_.size());
  if (data_len == 0) {
    if (finish) {
      std::string message(pending_read_data_.begin(), pending_read_data_.end());
      pending_read_data_.clear();
      pending_read_data_index_ = 0;
      delegate_->OnRelayMessage(message);
    }
    return;
  }
  const size_t old_size = pending_read_data_index_;
  const size_t new_size = old_size + data_len;
  if ((type != network::mojom::WebSocketMessageType::TEXT &&
       type != network::mojom::WebSocketMessageType::CONTINUATION) ||
      data_len > std::numeric_limits<uint32_t>::max() || new_size < old_size ||
      new_size > kMaxIncomingMessageSize) {
    Close(0, 0);
    return;
  }
  pending_read_data_.resize(new_size);
  pending_read_finished_ = finish;
  client_receiver_.Pause();
  ReadFromDataPipe(MOJO_RESULT_OK, mojo::HandleSignalsState());
}

void SyncRelay::OnDropChannel(bool was_clean, uint16_t code, const std::string& reason) {
  DCHECK_CALLED_ON_VALID_SEQUENCE(sequence_checker_);
  Close(code, 0);
}

void SyncRelay::OnClosingHandshake() {}

void SyncRelay::ReadFromDataPipe(MojoResult, const mojo::HandleSignalsState&) {
  CHECK_LT(pending_read_data_index_, pending_read_data_.size());
  size_t read = 0;
  const MojoResult result = readable_->ReadData(
      MOJO_READ_DATA_FLAG_NONE, base::span(pending_read_data_).subspan(pending_read_data_index_),
      read);
  if (result == MOJO_RESULT_SHOULD_WAIT) {
    readable_watcher_.ArmOrNotify();
    return;
  }
  if (result != MOJO_RESULT_OK) {
    Close(0, 0);
    return;
  }
  pending_read_data_index_ += read;
  if (pending_read_data_index_ < pending_read_data_.size()) {
    readable_watcher_.ArmOrNotify();
    return;
  }
  client_receiver_.Resume();
  if (pending_read_finished_) {
    std::string message(pending_read_data_.begin(), pending_read_data_.end());
    pending_read_data_.clear();
    pending_read_data_index_ = 0;
    pending_read_finished_ = false;
    // The delegate may destroy this.
    delegate_->OnRelayMessage(message);
  }
}

}  // namespace browser_core
