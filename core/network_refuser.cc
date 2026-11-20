#include "browser_core/network_refuser.h"

#include <utility>

#include "mojo/public/cpp/bindings/remote.h"
#include "net/cookies/cookie_partition_key.h"
#include "net/ssl/ssl_cert_request_info.h"
#include "net/ssl/ssl_info.h"

namespace browser_core {

NetworkRefuser::NetworkRefuser() = default;
NetworkRefuser::~NetworkRefuser() = default;

mojo::PendingRemote<network::mojom::URLLoaderNetworkServiceObserver> NetworkRefuser::Bind() {
  mojo::PendingRemote<network::mojom::URLLoaderNetworkServiceObserver> remote;
  receivers_.Add(this, remote.InitWithNewPipeAndPassReceiver());
  return remote;
}

void NetworkRefuser::OnSSLCertificateError(const GURL& url,
                                           int net_error,
                                           const net::SSLInfo& ssl_info,
                                           bool fatal,
                                           OnSSLCertificateErrorCallback response) {
  std::move(response).Run(net_error);
}

void NetworkRefuser::OnCertificateRequested(
    const std::optional<base::UnguessableToken>& window_id,
    const scoped_refptr<net::SSLCertRequestInfo>& cert_info,
    mojo::PendingRemote<network::mojom::ClientCertificateResponder> responder) {
  mojo::Remote<network::mojom::ClientCertificateResponder>(std::move(responder))
      ->ContinueWithoutCertificate();
}

void NetworkRefuser::OnAuthRequired(
    const std::optional<base::UnguessableToken>& window_id,
    int32_t request_id,
    const GURL& url,
    bool first_auth_attempt,
    bool do_not_prompt_for_login,
    const net::AuthChallengeInfo& auth_info,
    const scoped_refptr<net::HttpResponseHeaders>& head_headers,
    mojo::PendingRemote<network::mojom::AuthChallengeResponder> responder) {
  mojo::Remote<network::mojom::AuthChallengeResponder>(std::move(responder))
      ->OnAuthCredentials(std::nullopt);
}

void NetworkRefuser::OnLocalNetworkAccessPermissionRequired(
    network::mojom::TransportType type,
    network::mojom::IPAddressSpace ip_address_space,
    OnLocalNetworkAccessPermissionRequiredCallback callback) {
  std::move(callback).Run(network::mojom::LocalNetworkAccessResult::kDenied);
}

void NetworkRefuser::OnPlatformLocalNetworkPermissionRequired(
    OnPlatformLocalNetworkPermissionRequiredCallback callback) {
  std::move(callback).Run(false);
}

void NetworkRefuser::OnClearSiteData(
    const GURL& url,
    const std::optional<net::SchemefulSite>& top_level_site,
    const std::optional<base::UnguessableToken>& nonce,
    const std::string& header_value,
    int32_t load_flags,
    const std::optional<net::CookiePartitionKey>& cookie_partition_key,
    bool partitioned_state_allowed_only,
    OnClearSiteDataCallback callback) {
  std::move(callback).Run();
}

void NetworkRefuser::OnLoadingStateUpdate(network::mojom::LoadInfoPtr info,
                                          OnLoadingStateUpdateCallback callback) {
  std::move(callback).Run();
}

void NetworkRefuser::OnDataUseUpdate(int32_t network_traffic_annotation_id_hash,
                                     base::ByteSize recv_bytes,
                                     base::ByteSize sent_bytes) {}

void NetworkRefuser::Clone(
    mojo::PendingReceiver<network::mojom::URLLoaderNetworkServiceObserver> observer) {
  receivers_.Add(this, std::move(observer));
}

void NetworkRefuser::OnWebSocketConnectedToLocalNetwork(
    const GURL& request_url,
    network::mojom::IPAddressSpace ip_address_space) {}

void NetworkRefuser::OnUrlLoaderConnectedToLocalNetwork(
    const GURL& request_url,
    network::mojom::IPAddressSpace response_address_space,
    network::mojom::IPAddressSpace client_address_space,
    network::mojom::IPAddressSpace target_address_space) {}

}  // namespace browser_core
