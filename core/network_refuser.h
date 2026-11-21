// Answers what the profile's own network observer would answer for the user with a refusal, for
// a request the core makes on its own, as a link card's or a shared link's: no client certificate,
// a cancel for every sign-in and certificate error, no private network permission, and no site's
// data cleared. A loader factory made with Bind() asks it in place of the profile's.

#ifndef BROWSER_CORE_NETWORK_REFUSER_H_
#define BROWSER_CORE_NETWORK_REFUSER_H_

#include <optional>

#include "base/unguessable_token.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/receiver_set.h"
#include "services/network/public/mojom/url_loader_network_service_observer.mojom.h"
#include "url/gurl.h"

namespace browser_core {

class NetworkRefuser : public network::mojom::URLLoaderNetworkServiceObserver {
 public:
  NetworkRefuser();
  NetworkRefuser(const NetworkRefuser&) = delete;
  NetworkRefuser& operator=(const NetworkRefuser&) = delete;
  ~NetworkRefuser() override;

  mojo::PendingRemote<network::mojom::URLLoaderNetworkServiceObserver> Bind();
  void OnSSLCertificateError(const GURL& url,
                             int net_error,
                             const net::SSLInfo& ssl_info,
                             bool fatal,
                             OnSSLCertificateErrorCallback response) override;
  void OnCertificateRequested(
      const std::optional<base::UnguessableToken>& window_id,
      const scoped_refptr<net::SSLCertRequestInfo>& cert_info,
      mojo::PendingRemote<network::mojom::ClientCertificateResponder> responder) override;
  void OnAuthRequired(
      const std::optional<base::UnguessableToken>& window_id,
      int32_t request_id,
      const GURL& url,
      bool first_auth_attempt,
      bool do_not_prompt_for_login,
      const net::AuthChallengeInfo& auth_info,
      const scoped_refptr<net::HttpResponseHeaders>& head_headers,
      mojo::PendingRemote<network::mojom::AuthChallengeResponder> responder) override;
  void OnLocalNetworkAccessPermissionRequired(
      network::mojom::TransportType type,
      network::mojom::IPAddressSpace ip_address_space,
      OnLocalNetworkAccessPermissionRequiredCallback callback) override;
  void OnPlatformLocalNetworkPermissionRequired(
      OnPlatformLocalNetworkPermissionRequiredCallback callback) override;
  void OnClearSiteData(const GURL& url,
                       const std::optional<net::SchemefulSite>& top_level_site,
                       const std::optional<base::UnguessableToken>& nonce,
                       const std::string& header_value,
                       int32_t load_flags,
                       const std::optional<net::CookiePartitionKey>& cookie_partition_key,
                       bool partitioned_state_allowed_only,
                       OnClearSiteDataCallback callback) override;
  void OnLoadingStateUpdate(network::mojom::LoadInfoPtr info,
                            OnLoadingStateUpdateCallback callback) override;
  void OnDataUseUpdate(int32_t network_traffic_annotation_id_hash,
                       base::ByteSize recv_bytes,
                       base::ByteSize sent_bytes) override;
  void Clone(
      mojo::PendingReceiver<network::mojom::URLLoaderNetworkServiceObserver> observer) override;
  void OnWebSocketConnectedToLocalNetwork(const GURL& request_url,
                                          network::mojom::IPAddressSpace ip_address_space) override;
  void OnUrlLoaderConnectedToLocalNetwork(
      const GURL& request_url,
      network::mojom::IPAddressSpace response_address_space,
      network::mojom::IPAddressSpace client_address_space,
      network::mojom::IPAddressSpace target_address_space) override;

 private:
  mojo::ReceiverSet<network::mojom::URLLoaderNetworkServiceObserver> receivers_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_NETWORK_REFUSER_H_
