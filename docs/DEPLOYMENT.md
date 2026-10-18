# Private deployment and recovery

Little Wardrobe runs as a single Rust container on the family’s LAN/VPN. Building the package or rendering templates does not deploy the live homelab. The application requires family accounts on the private network.

## Package and configuration

Build `nix build .#container --out-link result-container` and load it with `docker load -i result-container`. The image is `ghcr.io/fruitiex/littlewardrobe:main`, runs as UID/GID 10001 and listens on 8080. Its frontend is bundled, and TLS trust and timezone files are included.

Persist `/data`; temporary files use `/tmp`. Set `LITTLEWARDROBE_ORIGIN=https://littlewardrobe.fruitiex.org` or your actual HTTPS origin. Same-origin protection and secure cookies depend on that value. Optional configuration:

| Variable                             | Purpose                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `LITTLEWARDROBE_BIND`                | Listener; container defaults to `0.0.0.0:8080`                                                 |
| `LITTLEWARDROBE_DATA_DIR`            | Persistent directory; container defaults to `/data`                                            |
| `LITTLEWARDROBE_SETUP_TOKEN`         | Random first-owner token, at least 16 characters; generated privately on first start if absent |
| `LITTLEWARDROBE_SECRET_KEY`          | Exactly 64 hexadecimal characters; encryption key for provider credentials                     |
| `LITTLEWARDROBE_SQLITE_JOURNAL_MODE` | `WAL` for local/block storage, `DELETE` for an appropriately configured NFS volume             |

Use setup and encryption secrets unique to this installation. Preserve the encryption key to decrypt the saved AI credential. Changing it is not a credential rotation strategy.

## Kubernetes and private ingress

Templates are in `deploy/kubernetes/apps/littlewardrobe`. The HTTPRoute attaches only to `private` in `envoy-gateway-system`, section `https`. There is no public gateway parent. Configure DNS to the private gateway, confirm the listener’s reachability from the LAN/VPN, and ensure external clients cannot reach it. A private parent name alone does not replace routing/firewall configuration.

HTTPS enables installation, camera input and clipboard behavior on phones. Install from the browser’s Add to home screen menu. Shared inventory and queued edits survive connection loss; uploads, AI and exports require connectivity. Private API images are not cached by the service worker.

The deployment uses one replica and Recreate, a non-root process, a read-only root filesystem, readiness/liveness probes, a persistent volume and limited temporary storage. Verify the storage class in `pvc.yaml` before use. `syno-storage` is the inherited block-storage default. If choosing NFS, set journal mode `DELETE`, use reliable NFSv4 locking/sync and keep one replica; SQLite WAL is unsuitable for the NFS shared-memory index.

The base kustomization excludes the secret so it renders before credentials exist:

```sh
nix develop --command kustomize build deploy/kubernetes/apps/littlewardrobe
```

Copy `secret.sops.yaml.example` to a private file, replace both values, encrypt with the homelab’s SOPS recipients and reference it through the Flux overlay performing SOPS decryption. Never commit plaintext values. The deployment expects `littlewardrobe-server-env`. The repository and GHCR package are private. Configure an existing GHCR pull secret through `spec.template.spec.imagePullSecrets` or the namespace service account before rollout. Use an immutable image digest for a real rollout. Domain, storage and secrets are deployment configuration.

## Accounts and operation

Retrieve the first-owner token through trusted server access, create the owner, save the recovery code and invite the second adult. Invitations expire after seven days. Each account has a separate session and can sign out, edit its profile and manage its password. The owner manages invitations, household name, AI configuration and backups. Recovery rotates credentials and signs out old sessions.

`/api/health` checks the process and `/api/ready` checks the database. Preserve long-lived SSE responses through the proxy; the HTTPRoute has no request timeout. Reconnection refreshes authoritative state. Conflicting changes stay in the local outbox for explicit retry/discard through the connection controls.

## Backup and recovery

Settings offers a household JSON backup containing inventory, children, locations, sale drafts, history, normalized images and photo provenance. Accounts and provider secrets are excluded. Restore requires the owner, validates data/images, remaps image IDs and rejects a changed revision. Download the current backup before replacing a household.

For full recovery, retain consistent SQLite backups plus the server encryption key and other persisted configuration. The binary supports:

```sh
LITTLEWARDROBE_DATA_DIR=/data littlewardrobe-api --backup /backup/littlewardrobe.sqlite3
```

Choose a new destination; existing files are refused. This uses SQLite’s backup API while the app runs. Copying only the main database file while WAL is active is not a consistent backup. Pass the same data directory and mounted path when invoking from a separate host.

To recover the instance, stop it, restore into `/data/littlewardrobe.sqlite3`, preserve/restore the encryption key, correct UID/GID permissions, then start a compatible version. Confirm accounts, photos and AI connection. Back up before upgrading; migrations are automatic and a newer schema refuses downgrade. Schema 7 adds the durable clothing-photo queue and AI review metadata; schema 8 preserves Quick add starting values and request identity across retries. Pending tasks resume after restart; restoring a household JSON backup cancels tasks while preserving its clothing drafts and original photos.

## Homelab rollout — 2026-10-02

At the user's explicit request, the app is deployed at **https://littlewardrobe.fruitiex.org** through the private gateway only. DNS resolves to `192.168.11.99`; use the LAN/VPN. The public gateway and WAN returned 404 for this host. Homelab revision `273db55` deployed the successful CI `36922591167` release, source `c4cb95e`, pinned to digest `sha256:1bc82975fe97a170ac5ee206a1caf25a01b9ea5f9df5828f785601f154dcca54`.

The live configuration is in `~/projects/homelab/apps/littlewardrobe`, registered through the homelab Flux Kustomization. It uses existing `ghcr-credentials`, freshly SOPS-encrypted bootstrap/encryption secrets, and the retained 5 GiB `littlewardrobe-data-nfs` claim with `nfs-csi` and DELETE journaling. The NAS directory is `/volume3/homelab/littlewardrobe-data-nfs` on `nectarine.internal.fruitiex.org`. This deployment has independent accounts, a dedicated database and its own AI configuration. App CI publishes images but does not automatically deploy them; update the homelab's pinned digest for future rollouts.

Verification passed: Flux Ready/Healthy, one ready replica using the expected digest, bound storage, private route Accepted/ResolvedRefs, trusted HTTPS, normal DNS readiness, anonymous API/photo/SSE denial, cross-origin write rejection, public/WAN exclusion, and mobile/desktop first-use setup plus PWA checks. A consistent initial backup at `/data/initial-deployment-2026-10-02.sqlite3` passed SQLite integrity checks and confirmed DELETE mode. No accounts or family inventory were created during verification.

Retrieve the bootstrap token in your own private terminal, then create the first owner and save the recovery code:

```sh
cd ~/projects/homelab
nix-shell
sops --decrypt --extract '["stringData"]["LITTLEWARDROBE_SETUP_TOKEN"]' apps/littlewardrobe/secret.sops.yaml
```

Invite the second adult through Settings and configure Family. Connect AI through Settings using the [AI setup guide](AI.md). Preserve the SOPS encryption secret with database backups.
