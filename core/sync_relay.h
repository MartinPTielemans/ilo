// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Adapted from Chromium's components/streaming_client/streaming_websocket_client.h, at tag
// 156.0.8078.47: the account's relay connection (ilo-server's docs/protocol.md, Relay), whose
// frames are JSON text where that client's are binary, opened with the account's access token.

#ifndef BROWSER_CORE_SYNC_RELAY_H_
#define BROWSER_CORE_SYNC_RELAY_H_

#include <cstdint>
#include <string>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "base/sequence_checker.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "mojo/public/cpp/system/data_pipe.h"
#include "mojo/public/cpp/system/simple_watcher.h"
#include "services/network/public/mojom/websocket.mojom.h"
#include "url/gurl.h"

namespace browser_core {

class SyncRelay : public network::mojom::WebSocketHandshakeClient,
                  public network::mojom::WebSocketClient {
 public:
  class Delegate {
   public:
    virtual ~Delegate() = default;
    virtual void OnRelayOpen() = 0;
    // A whole text frame. The relay may be destroyed in the call.
    virtual void OnRelayMessage(const std::string& message) = 0;
    // The connection closed or failed, with the close code the server gave, such as 4001 for a
    // device signed out, or 0 if none; |response_code| is the handshake's HTTP status, such as 401
    // for an expired token, or 0. Nothing more comes, and the relay may be destroyed in the call.
    virtual void OnRelayClosed(uint16_t code, int response_code) = 0;
  };

  // Connects to |url|, the server's /relay as ws: or wss:, with |token| as its bearer.
  SyncRelay(const GURL& url, const std::string& token, Delegate* delegate);
  SyncRelay(const SyncRelay&) = delete;
  SyncRelay& operator=(const SyncRelay&) = delete;
  ~SyncRelay() override;

  // Sends |message| as one text frame, once open; before then it's dropped.
  void Send(const std::string& message);

 private:
  void ReadFromDataPipe(MojoResult result, const mojo::HandleSignalsState& state);
  void Close(uint16_t code, int response_code);

  // network::mojom::WebSocketHandshakeClient:
  void OnOpeningHandshakeStarted(network::mojom::WebSocketHandshakeRequestPtr request) override;
  void OnFailure(const std::string& message, int net_error, int response_code) override;
  void OnConnectionEstablished(
      mojo::PendingRemote<network::mojom::WebSocket> socket,
      mojo::PendingReceiver<network::mojom::WebSocketClient> client_receiver,
      network::mojom::WebSocketHandshakeResponsePtr response,
      mojo::ScopedDataPipeConsumerHandle readable,
      mojo::ScopedDataPipeProducerHandle writable) override;

  // network::mojom::WebSocketClient:
  void OnDataFrame(bool finish, network::mojom::WebSocketMessageType type, uint64_t data_len)
      override;
  void OnDropChannel(bool was_clean, uint16_t code, const std::string& reason) override;
  void OnClosingHandshake() override;

  raw_ptr<Delegate> delegate_;
  bool open_ = false;
  bool closed_ = false;

  std::vector<uint8_t> pending_read_data_;
  size_t pending_read_data_index_ = 0;
  bool pending_read_finished_ = false;

  mojo::Receiver<network::mojom::WebSocketHandshakeClient> handshake_receiver_{this};
  mojo::Receiver<network::mojom::WebSocketClient> client_receiver_{this};
  mojo::Remote<network::mojom::WebSocket> websocket_;
  mojo::ScopedDataPipeConsumerHandle readable_;
  mojo::SimpleWatcher readable_watcher_;
  mojo::ScopedDataPipeProducerHandle writable_;

  SEQUENCE_CHECKER(sequence_checker_);
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SYNC_RELAY_H_
