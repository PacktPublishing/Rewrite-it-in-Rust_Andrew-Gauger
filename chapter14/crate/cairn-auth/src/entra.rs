//! The OpenID Connect client for Entra ID. `cairn-auth` is a confidential
//! client: it holds the client secret the desktop app can't.

use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreErrorResponseType,
    CoreGenderClaim, CoreJsonWebKey,
    CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreResponseType, CoreRevocableToken,
    CoreRevocationErrorResponse, CoreTokenIntrospectionResponse,
    CoreTokenType,
};
use openidconnect::{
    reqwest, AdditionalClaims, AuthenticationFlow, AuthorizationCode,
    Client, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
    EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields,
    IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, StandardErrorResponse, StandardTokenResponse,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

/// The two claims Entra adds that Cairn needs: the person's object ID,
/// which never changes, and the app roles assigned to them.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EntraClaims {
    pub oid: Uuid,
    #[serde(default)]
    pub roles: Vec<String>,
}
impl AdditionalClaims for EntraClaims {}

type EntraTokenResponse = StandardTokenResponse<
    IdTokenFields<
        EntraClaims,
        EmptyExtraTokenFields,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >,
    CoreTokenType,
>;

type EntraClient = Client<
    EntraClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    EntraTokenResponse,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// What Entra said about the person who signed in.
#[derive(Debug)]
pub struct SignedIn {
    pub oid: Uuid,
    pub upn: String,
    pub display_name: String,
    pub roles: Vec<String>,
}

/// What `cairn-auth` remembers between sending the browser to Entra and
/// Entra sending it back.
pub struct Outbound {
    pub nonce: Nonce,
    pub pkce: PkceCodeVerifier,
}

pub struct Entra {
    client: EntraClient,
    http: reqwest::Client,
}

impl Entra {
    /// Reads Entra's discovery document for the tenant.
    pub async fn discover(
        issuer: &str,
        client_id: &str,
        client_secret: &str,
        redirect: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(issuer.to_owned())?,
            &http,
        )
        .await?;
        let client = EntraClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.to_owned()),
            Some(ClientSecret::new(client_secret.to_owned())),
        )
        .set_redirect_uri(RedirectUrl::new(redirect.to_owned())?);
        Ok(Entra { client, http })
    }

    /// The Entra sign-in URL, its CSRF state, and what to keep for the
    /// callback.
    pub fn start(&self) -> (Url, CsrfToken, Outbound) {
        let (challenge, pkce) =
            PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("email".into()))
            .set_pkce_challenge(challenge)
            .url();
        (url, state, Outbound { nonce, pkce })
    }

    /// Trades Entra's code for tokens, and checks the ID token's
    /// signature, issuer, audience, expiry and nonce.
    pub async fn finish(
        &self,
        code: String,
        outbound: Outbound,
    ) -> Result<SignedIn, Box<dyn std::error::Error>> {
        let tokens = self
            .client
            .exchange_code(AuthorizationCode::new(code))?
            .set_pkce_verifier(outbound.pkce)
            .request_async(&self.http)
            .await?;
        let id_token = tokens
            .extra_fields()
            .id_token()
            .ok_or("Entra returned no ID token")?;
        let claims = id_token.claims(
            &self.client.id_token_verifier(),
            &outbound.nonce,
        )?;
        let upn = claims
            .preferred_username()
            .map(|u| u.to_string())
            .ok_or("ID token has no preferred_username")?;
        let display_name = claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.to_string())
            .unwrap_or_else(|| upn.clone());
        Ok(SignedIn {
            oid: claims.additional_claims().oid,
            upn,
            display_name,
            roles: claims.additional_claims().roles.clone(),
        })
    }
}
