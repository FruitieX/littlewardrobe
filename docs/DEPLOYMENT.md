# Private deployment and recovery

Little Wardrobe runs as a single Rust container on a private network. Building the package or rendering templates does not deploy anything; the live deployment is managed in the [homelab](https://github.com/FruitieX/homelab) repository. The application requires family accounts on the private network.

## Package and configuration

Build `nix build .#container --out-link result-container` and load it with `docker load -i result-container`. The image is `ghcr.io/fruitiex/littlewardrobe:main`, runs as UID/GID 10001 and listens on 8080. Its frontend is bundled, and TLS trust and timezone files are included.

Persist `/data`; temporary files use `/tmp`. Set `LITTLEWARDROBE_ORIGIN` to your actual HTTPS origin, e.g. `https://littlewardrobe.example.com`. Same-origin protection and secure cookies depend on that value. Optional configuration:

| Variable                             | Purpose                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `LITTLEWARDROBE_BIND`                | Listener; container defaults to `0.0.0.0:8080`                                                 |
| `LITTLEWARDROBE_DATA_DIR`            | Persistent directory; container defaults to `/data`                                            |
| `LITTLEWARDROBE_SETUP_TOKEN`         | Random first-owner token, at least 16 characters; generated privately on first start if absent |
| `LITTLEWARDROBE_SECRET_KEY`          | Exactly 64 hexadecimal characters; encryption key for provider credentials                     |
| `LITTLEWARDROBE_SQLITE_JOURNAL_MODE` | `WAL` for local/block storage, `DELETE` for an appropriately configured NFS volume             |
| `LITTLEWARDROBE_OIDC_ISSUER`         | Optional passkey sign-in: OpenID Connect issuer, e.g. `https://id.example.com`                 |
| `LITTLEWARDROBE_OIDC_CLIENT_ID`      | Client id registered at the provider                                                           |
| `LITTLEWARDROBE_OIDC_CLIENT_SECRET`  | Client secret; keep it in the SOPS secret                                                      |
| `LITTLEWARDROBE_OIDC_REDIRECT_URL`   | Defaults to `$LITTLEWARDROBE_ORIGIN/api/auth/oidc/callback`; register it at the provider       |
| `LITTLEWARDROBE_OIDC_ALLOWED_GROUPS` | Comma-separated groups; sign-in requires one of them                                           |
| `LITTLEWARDROBE_OIDC_USER_MAP`       | `provideruser:local@email,...` links existing password accounts on their first passkey sign-in |

Use setup and encryption secrets unique to this installation. Preserve the encryption key to decrypt the saved AI credential. Changing it is not a credential rotation strategy.

## Kubernetes and private ingress

Templates are in `deploy/kubernetes/apps/littlewardrobe`. The HTTPRoute attaches only to a gateway named `private`, section `https`. There is no public gateway parent. Configure DNS to the private gateway, confirm the listener’s reachability from your private network, and ensure external clients cannot reach it. A private parent name alone does not replace routing/firewall configuration.

HTTPS enables installation, camera input and clipboard behavior on phones. Install from the browser’s Add to home screen menu. Shared inventory and queued edits survive connection loss; uploads, AI and exports require connectivity. Private API images are not cached by the service worker.

The deployment uses one replica and Recreate, a non-root process, a read-only root filesystem, readiness/liveness probes, a persistent volume and limited temporary storage. Verify the storage class in `pvc.yaml` before use. The storage class in the template is an example block-storage class; use your own. If choosing NFS, set journal mode `DELETE`, use reliable NFSv4 locking/sync and keep one replica; SQLite WAL is unsuitable for the NFS shared-memory index.

The base kustomization excludes the secret so it renders before credentials exist:

```sh
nix develop --command kustomize build deploy/kubernetes/apps/littlewardrobe
```

Copy `secret.sops.yaml.example` to a private file, replace both values, encrypt with the homelab’s SOPS recipients and reference it through the Flux overlay performing SOPS decryption. Never commit plaintext values. The deployment expects `littlewardrobe-server-env`. If the GHCR package is private, configure a pull secret through `spec.template.spec.imagePullSecrets` or the namespace service account before rollout. Use an immutable image digest for a real rollout. Domain, storage and secrets are deployment configuration.

## Accounts and operation

Retrieve the first-owner token through trusted server access, create the owner, save the recovery code and invite the second adult. Invitations expire after seven days. Each account has a separate session and can sign out, edit its profile and manage its password. The owner manages invitations, household name, AI configuration and backups. Recovery rotates credentials and signs out old sessions.

Passkey sign-in through an OpenID Connect provider (Pocket ID) is optional and enabled only when the issuer, client id and client secret are all set. The sign-in screen then leads with “Sign in with passkey” and keeps email/password sign-in as a quieter fallback; a device that signs in with a password goes straight to the password form next time. Identities are keyed by issuer and subject. A first sign-in uses an existing link, then the user map; anyone else in an allowed group becomes a password-less member of the household (only when exactly one household exists). An account linked to one provider identity is never linked to a second. Passkey sessions last 365 days and slide while in use; password sessions keep their 30-day lifetime, and companion bearer tokens are unaffected. Owner settings can remove such members, or run `kubectl exec deploy/littlewardrobe -- littlewardrobe-api --remove-member EMAIL`. `npm run test:oidc` runs the flow against a mock provider in the production build.

`/api/health` checks the process and `/api/ready` checks the database. Preserve long-lived SSE responses through the proxy; the HTTPRoute has no request timeout. Reconnection refreshes authoritative state. Conflicting changes stay in the local outbox for explicit retry/discard through the connection controls.

## Backup and recovery

Settings offers a household JSON backup containing inventory, children, locations, sale drafts, history, normalized images and photo provenance. Accounts and provider secrets are excluded. Restore requires the owner, validates data/images, remaps image IDs and rejects a changed revision. Download the current backup before replacing a household.

For full recovery, retain consistent SQLite backups plus the server encryption key and other persisted configuration. The binary supports:

```sh
LITTLEWARDROBE_DATA_DIR=/data littlewardrobe-api --backup /backup/littlewardrobe.sqlite3
```

Choose a new destination; existing files are refused. This uses SQLite’s backup API while the app runs. Copying only the main database file while WAL is active is not a consistent backup. Pass the same data directory and mounted path when invoking from a separate host.

To recover the instance, stop it, restore into `/data/littlewardrobe.sqlite3`, preserve/restore the encryption key, correct UID/GID permissions, then start a compatible version. Confirm accounts, photos and AI connection. Back up before upgrading; migrations are automatic and a newer schema refuses downgrade. Schema 7 adds the durable clothing-photo queue and AI review metadata; schema 8 preserves Quick add starting values and request identity across retries. Pending tasks resume after restart; restoring a household JSON backup cancels tasks while preserving its clothing drafts and original photos.
