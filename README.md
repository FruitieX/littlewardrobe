# Little Wardrobe

A private, shared family wardrobe. Add a garment photo and optional tag photo, review AI-suggested details, then track rotation, storage, hand-me-downs and resale. The interface is in English and sale listings are in Finnish.

Children, birthdays, clothing/shoe sizes and storage locations are configurable. Search by child, lifecycle, category, size, season and location. Preserve purchase history, condition, defects, private notes, sold/donated items and keepsakes. Numeric size comparisons suggest pieces worth checking without pretending to know their actual fit.

The sale desk supports single items and bundles, AI or manual drafts, asking prices, approval, original-photo exports, clipboard and supported file sharing, external listing links and actual sale amounts. Changes reset approval. Selling an item flags overlapping listings for removal. AI prices are unverified estimates. Direct posting is unavailable for ordinary family accounts through the verified official interfaces; see [marketplace feasibility](docs/MARKETPLACES.md). Vinted uses original photos only.

The family starts empty. [Screenshots](mockups/README.md) use isolated test data and a controlled provider.

## Run locally

```sh
nix develop
command -v node npm cargo cc python3
npm ci
npm run dev
# In another Nix shell:
cargo run --manifest-path backend/Cargo.toml
```

Open `http://localhost:3101`. The backend listens at `127.0.0.1:3002`. Find the first-owner token in `data/setup-token`, create your account, save the recovery code and invite your wife through Settings.

Connect an OpenAI-compatible vision model or pair a personal Hermes companion in Settings. Credentials remain server-side or on the agent machine; uploaded photos remain private to the household. An optional reference-image model enables reviewed background edits. See [AI setup](docs/AI.md).

## Build and self-host

```sh
nix build .#container --out-link result-container
docker load -i result-container
nix develop --command npm run test:container
```

The unprivileged container serves frontend and API on port 8080. Persist `/data`, configure the actual HTTPS origin and use the supplied [private ingress](deploy/kubernetes/apps/littlewardrobe/ingress.yaml) on the private gateway. See [deployment and backups](docs/DEPLOYMENT.md). The live cluster is not changed by building this repo.

## Verify

```sh
nix develop --command cargo fmt --manifest-path backend/Cargo.toml -- --check
nix develop --command cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
nix develop --command cargo test --manifest-path backend/Cargo.toml
nix develop --command npm run typecheck
nix develop --command npm test
nix develop --command python3 scripts/hermes-companion.check.py
nix develop --command npm run format:check
nix develop --command npm run build
nix develop --command npm run test:production
nix develop --command npm run test:oidc
```

The production browser suite starts isolated backend/provider servers. It exercises two adult sessions, mobile family/storage setup, clothing/tag photos, AI review, reference-image edits, hand-me-downs, offline reload/reconnect, concurrent editor conflicts, sale approval/export/edits, actual sales, bundles, backups and responsive screens. Rust checks cover household isolation, photo provenance, stale writes, persistence, approvals, provider requests and inherited auth/sync behavior.

TanStack Start/Router, React, TypeScript, Tailwind and Radix; Rust, Tokio, Axum, SQLite, SSE and ts-rs. Authenticated households, private images, encrypted AI credentials, guarded concurrent edits and a persisted offline outbox protect family data. [The plan](docs/PLAN.md) records requirements and progress.

DM Sans and Fraunces include their SIL Open Font License files. The legacy server icon reference comes from Twemoji, with its CC BY 4.0 notice in `backend/assets/LICENSE-GRAPHICS.txt`; that icon feature is not exposed in the clothing interface. App marks are repo-native SVGs.
