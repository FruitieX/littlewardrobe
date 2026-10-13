# Little Wardrobe product and implementation plan

Build a working family application in `/home/rasse/littlewardrobe` to manage the children’s clothes from purchase through daily use, storage, hand-me-downs and resale. The user requested a complete application with AI assistance for photo intake and sale preparation. This document preserves the agreed scope, practical constraints, implementation decisions and progress so work can continue across sessions.

## Agreed requirements

- Build a working app, not only a product proposal.
- Family-only, mobile-friendly web app, installable as a PWA.
- English interface and Finnish sale listings.
- Self-host with configurable AI connections, including an OpenAI-compatible endpoint and the personal Hermes companion.
- Full scope: inventory, photo intake, configurable children, active rotation, storage, hand-me-downs, sale drafts, bundles and posting automation where feasible.
- Prepare a private ingress so the application is accessible only through the family’s LAN or VPN. Do not add a public gateway. Checked updates to the existing private homelab instance are authorized; preserve the private ingress.
- Save this plan and make its completion the active goal, as requested by the user.

The working name is **Little Wardrobe**. It is an implementation default, not a user-approved brand or a claim of name availability. Real child names, birthdays, sizes and inventory start empty and are configured through the app.

## Photo intake and AI review

The primary flow is take or choose a clothing photo, optionally attach a separate tag photo, request AI suggestions, review uncertain details, assign a child and storage location, then save. Support front, back, tag and detail photographs on the same inventory entry. Keep original photographs distinguishable from optional edited images.

AI suggests descriptive name, clothing category, visible brand, printed size and size system, color, material where readable, likely season and visible condition. Show field evidence and uncertainty. An unreadable size triggers a request for a tag photo or manual entry; do not infer size from garment proportions or child age. Condition from a photograph is provisional. Keep visible defects and allow the family to confirm wear, stains, pilling and repairs. Purchase price, purchase date and purchase source remain optional manual facts unless evidence is explicitly supplied. Do not invent provenance, authenticity, waterproof performance or home conditions.

Support manual intake when AI is disconnected or unavailable. Errors must preserve the draft and uploaded images. Photo upload and AI calls require connectivity; saved inventory edits use a persisted offline outbox. Initial support is JPEG, PNG and WebP, with clear limits; HEIC is a possible later enhancement rather than a promised first-release capability.

## Inventory and family model

Each garment has a unique ID, name, category, brand, printed size, size system, color, material, season, condition, defects, lifecycle status, current child, intended next child, storage location, optional purchase price/date/source, private notes, photographs and creation date.

Lifecycle states are in rotation, saved for later, ready to sell, reserved, sold, donated and keepsake. Child assignment is separate from lifecycle and location. Keep sold and donated records searchable rather than silently deleting history. Provide editable entries and guarded deletion. Hand-me-downs change the current child and rotation status explicitly.

Children have configurable names, optional birthdays, current clothing size, current EU shoe size and a visual color. Calculate age from the birthday. Support any practical family size; no hardcoded pair of children. Locations have a name and optional note, for example a drawer, attic box or daycare bag.

Provide searches and filters by child, status, category, size, season and location. Show rotation, stored and ready-to-sell counts. Explain suggestions for checking garments whose numeric labels fall below a child’s configured size; label fit as uncertain because brands vary. Compare like size systems only. Useful extensions include duplicate-item warnings, next-season coverage and QR labels for storage boxes, but they are not required for completion unless added to this plan explicitly.

## Sale preparation and tracking

Select one item or several items as a bundle, choose Tori or Vinted, then generate or manually create a Finnish title, description, total asking price and photo selection. Include known defects. Keep private child details out of generated listing context. Prices are unverified estimates unless a permitted, verifiable market-data integration actually supplies comparisons; never claim market research from a language-model guess.

Sale drafts are separate durable records that reference inventory items. Support draft, approved, listed and sold states, published listing URLs, actual sale price and sale date. Review size, confirmed condition, text, photos and price before approval. Changes to the approved content or relevant garment facts reset approval. Approval does not mean publication. Avoid listing the same item individually and in a live bundle on the same platform. Record sales and flag other listings of the sold items for removal to prevent double selling.

Provide copyable Finnish text, downloadable photographs and text, file sharing where the browser supports it, and links to the marketplace. Preserve a usable export path even if direct posting is unavailable.

Optional image assistance must edit an actual garment photograph, preserve shape, color, material and every visible defect, and require comparison against the original. Edited images stay separate. Never synthesize an idealized replacement garment for a factual sale listing. The current [Vinted catalogue rules](https://www.vinted.fi/catalog-rules), checked on 2026-10-01, require seller-taken photographs and state that image editing is not allowed. Therefore Vinted exports use originals only. The image-edit route is provider-dependent and must report unsupported capability honestly.

Investigate current Tori and Vinted posting interfaces and automation terms. Implement an approved posting integration only where a supported route is actually available. The acceptance requirement is a documented feasibility result and a reliable approved export workflow if personal-account posting is unsupported; do not invent an API, bypass account protections, or claim posting works without verification. If a permitted integration becomes available, require approval of the exact draft, idempotent publication and a stored result/link before marking it published.

## Architecture and access

Use TanStack Start/Router, React, TypeScript and accessible Radix dialogs for a calm clothing inventory. Use Rust, Tokio, Axum, SQLite and generated TypeScript API types.

Reuse authenticated household isolation, first-owner setup token, account recovery, expiring invitations, secure same-origin cookie sessions, encrypted AI credentials, private uploaded images, atomic operations, idempotency, guarded concurrent edits, SSE refresh and persisted offline changes. Keep API keys and companion credentials on the appropriate server/agent machine. Test clothing-specific input validation, household image ownership and malformed AI output. Do not assume that previously tested shopping behavior proves the clothing features work.

Use a project-local locked Nix environment, npm and Cargo lockfiles. Package frontend and backend into a single unprivileged container with persistent data and health checks. Provide a private Gateway API HTTPRoute using the homelab’s private gateway, HTTPS for PWA/camera behavior, a single replica, suitable SQLite storage, and SOPS secret examples. Document installation, provider setup, invitation, backups, recovery and upgrade procedures. Checked updates to the existing private homelab instance are authorized as of 2026-10-13.

Commit and push completed, checked task changes as authorized by `/home/rasse/AGENTS.md`. A new private repository may be created if needed to fulfill that standing push instruction. Include only changes made for this application.

## Completion checklist

- [x] Capture the user’s preferences, scope and implementation conventions.
- [x] Save this plan and activate completion as the goal.
- [x] Bootstrap the isolated application and verify tools in Nix.
- [x] Complete the English mobile interface for inventory, garment detail, photo intake, family, storage and sale desk.
- [x] Validate persistent clothing records, family configuration, hand-me-down transitions, inventory filters and sold/donated history.
- [x] Validate both adult accounts, independent-device synchronization, guarded concurrency, offline edits and household/photo isolation.
- [x] Complete and test reviewed AI clothing/tag extraction through configurable API and companion transports, including uncertainty and unavailable-provider behavior.
- [x] Complete and test Finnish single-item/bundle listings, transparent price suggestions, explicit approval, approval invalidation and sale tracking.
- [x] Complete and test original-photo export, supported browser sharing and optional provider image editing; preserve Vinted’s originals-only constraint.
- [x] Document posting feasibility with current primary sources and implement supported automation if available.
- [x] Provide installable PWA, private deployment templates, Nix container packaging and backup/recovery instructions.
- [x] Run appropriate Rust, TypeScript, frontend, companion and realistic production browser checks, including mobile screenshots and two-device/offline scenarios.
- [x] Review the actual screens, repair discovered issues, update this progress section, commit and push the finished application.

## Progress

The working application now includes the English mobile wardrobe, configurable children and storage, private photo/tag intake, evidence and uncertainty review, size checks, hand-me-downs, Finnish single-item and bundle drafts, approval, guarded exports, published links and actual sale history. The family starts empty. Household isolation and guarded, persisted offline edits protect shared family data.

Production browser checks have passed with two adult accounts and independent devices. They cover disjoint edits, rejected same-field conflicts, private photos, unreadable size followed by a tag photo, reference-image editing, offline reload/reconnect, original-only Vinted export, approval resets, sale tracking, bundle preparation, JSON backup/restore and 320–1440px layouts. Actual screenshots are saved in `mockups/` and have been visually reviewed. Browser AI fixtures and illustrated garments are isolated test data.

Delivery checks passed: 60 Rust tests, Clippy for all targets with warnings denied, Rust formatting, 28 frontend tests, TypeScript, five Python companion protocol tests, Prettier, the frontend/PWA build and private Kubernetes template rendering. Clothing tests cover private inline photos, reviewed drafts, offline companion errors, malformed output and edited-photo provenance. The final Nix container passed the complete production browser suite as UID 10001 with a read-only root and isolated temporary data. The image is available locally as `ghcr.io/fruitiex/littlewardrobe:main` and through the `result-container` build artifact.

The completed application is delivered in the private repository [FruitieX/littlewardrobe](https://github.com/FruitieX/littlewardrobe). The validation/publishing workflow builds and verifies the container before publishing to GHCR; it has no live deployment job. All accepted completion milestones are implemented and checked. External provider configuration, live deployment and any future business-only publisher are documented operational steps outside this delivery.

Primary marketplace documentation establishes business-only integration routes, not an available personal-account publisher. The accepted fallback is implemented and documented in `docs/MARKETPLACES.md`. Provider credentials and a live personal companion remain external configuration. Private deployment and recovery instructions are in `docs/DEPLOYMENT.md`; templates do not alter the live cluster. At the initial implementation delivery, no marketplace publication or live deployment had occurred. The later authorized homelab deployment is recorded below.

## Private homelab deployment — 2026-10-02

The user explicitly authorized deploying the application to the homelab with a private ingress. The target is `https://littlewardrobe.fruitiex.org`, attached only to the existing private Envoy gateway (`192.168.11.99`). Deployment uses the current homelab NFS CSI convention, a separate retained 5 GiB claim, SQLite DELETE journaling, fresh SOPS-encrypted bootstrap/encryption secrets, existing GHCR pull credentials and the immutable image from successful application CI `36922591167`.

- [x] Inspect the current homelab, update the clean checkout and verify cluster/tool access.
- [x] Prepare and validate private-only routing, retained storage, non-root deployment, SOPS round-trip and Kubernetes server schemas.
- [x] Commit/push the homelab configuration and reconcile through Flux.
- [x] Verify the running release, retained NFS data, trusted private HTTPS/PWA, anonymous access restrictions and public-gateway exclusion.
- [x] Record the deployment result and first-owner instructions, then commit/push final documentation.

Deployment result: homelab revision `273db55` reconciled successfully; Flux is Ready/Healthy with one ready replica, the exact published digest `sha256:1bc82975fe97a170ac5ee206a1caf25a01b9ea5f9df5828f785601f154dcca54` and a Bound retained NFS claim. The private HTTPRoute is Accepted/ResolvedRefs and DNS resolves to `192.168.11.99`. Trusted HTTPS and normal DNS readiness passed. Public gateway and WAN requests returned 404; anonymous private APIs/photos/SSE returned 401 and cross-origin writes returned 403. Mobile/desktop HTTPS setup and PWA checks passed without page errors or overflow. An initial consistent NFS database backup passed integrity and DELETE-journal checks. The family is empty and awaits first-owner setup; AI provider configuration remains external setup. First-login and operations instructions are in `~/homelab/apps/littlewardrobe/README.md`.

## Product copy cleanup — 2026-10-02

Removed references to the originating project from the README, deployment guide and plan so Little Wardrobe is described independently. A source and asset search confirmed the runtime application already uses its own branding. This change updates documentation only.

## Distinct visual identity and personal AI — 2026-10-02

The user requested a distinct visual identity and the same personal AI connection settings as the existing family application, with an independent Hermes profile and worker. The existing private deployment remains authorized.

- [x] Apply denim-blue and apricot styling, bold sans-serif headings, clothing illustrations and a consistent hanger/shirt app icon.
- [x] Verify the mobile, desktop, authentication and dialog screens and two-device production flows.
- [x] Publish and privately deploy the checked release.
- [x] Configure an independent personal Hermes profile/service, match production model settings and verify the live connection.

The visual refresh passed TypeScript, all 28 frontend tests, the PWA build and the complete two-device production browser suite. Screenshots include first-owner setup, photo/tag review, listing review and the actual mobile viewport. The profile is prepared with separate settings; a fresh personal sign-in is required because cloned single-use OAuth grants cannot safely be shared.

The dedicated profile is signed in, paired to the existing household and active as a sandboxed user service. Production models/chat, real tag vision and private image generation passed. Both family workers passed independent native token refresh and production chat afterward. The refresh check exposed and repaired a shared helper incompatibility: general Hermes CLI pool loading imports an extra borrowed login, so workers now refresh only their owned grant under the local auth lock. Twelve offline companion tests cover ownership and concurrent renewal. The administrator activated the built Pear system generation; the enabled worker is now preserved through boot.

Visual rollout completed through homelab revision `9fdc66e`, using application source `756b262` and immutable digest `sha256:5f0460674bae51c2d20e1301decbbcdaf4ba66d0a2128c01a1f5374076bd296e` from successful CI `36976223456`. Flux is Ready/Healthy, the exact image is running, and the HTTPRoute remains attached only to the private gateway with Accepted/ResolvedRefs conditions. Fresh 390px and 1440px HTTPS browser contexts verified the denim theme, bold headings, icon/manifest, active service worker, private session boundaries and no overflow or page errors. The production AI queue passed models/chat again after rollout. Consistent pre-upgrade backup `/data/pre-visual-20261002.sqlite3` passed integrity and DELETE journaling; all eight saved AI settings match. The fully built Pear generation is active with both workers enabled and no failed services. All follow-up milestones are complete.

## Photo suggestion validation repair — 2026-10-13

- [x] Reproduce the reported production photo suggestion failure with the sole catalog item's original photos through the dedicated personal AI transport.
- [x] Constrain category, size system, season and condition in the structured output schema using the saved-inventory choices, and clarify unknown-field defaults in the prompt.
- [x] Return a photo-AI-specific error for invalid provider drafts; preserve the editor draft and private photos on failure.
- [x] Verify the actual photos produce valid choices with the repaired schema, and test malformed season output, draft preservation and retry in the production browser suite.

The original prompt told the model to leave unknown text empty, and the schema accepted arbitrary strings even for enumerated fields. A repeat using the existing catalog photos returned an empty season, reproducing the generic clothing/child/location/date validation error. The repaired request returned season `all` with valid category, size system and condition. These diagnostic calls made no catalog changes and retained the existing personal credentials. The subsequently authorized private rollout is recorded below.

## Little Cupboard ports and private updates — 2026-10-13

The user authorized deploying the photo-suggestion repair, removed the repeated deployment approval requirement, and requested the useful Little Cupboard ports. Checked updates to the existing private instance are now authorized; LAN/VPN-only routing remains required.

- [x] Deploy and verify the checked photo-suggestion repair.
- [x] Add durable background clothing-photo intake, persistent AI-review markers, safe retry/cancel, and preservation of original photos.
- [x] Port separate camera/gallery controls, fixed dialog headers and accent-insensitive multiword search.
- [x] Port SQLite/rate-limit panic recovery and isolate background-worker failures.
- [x] Test two-device production browser behavior, sleeping/reopened pages, failures, cancellation and review; publish and privately deploy the checked ports.

Background intake adds a visible pending clothing entry immediately, finishes supported descriptive fields on the server, and leaves an AI review marker until an adult reviews and saves it. It never assigns a child/location, infers an unreadable size, overwrites concurrent manual edits or approves a sale listing. Original private photographs are retained. Shopping-specific pantry, geolocation and generated-icon features are outside this port.

Implementation validation: the background queue stores original-photo entries and task acceptance atomically, deduplicates retried requests, resumes interrupted work, records shared history/revisions and uses SSE refresh. It merges only unchanged descriptive fields and guards review, cancellation, deletion, photo replacement and JSON restore. Unreviewed clothing cannot approve a sale. New review metadata is defaulted when comparing older whole-record offline edits. Fixed headers, split photo controls and multiword accent-insensitive clothing search have been visually reviewed in real mobile/desktop screenshots. The expanded two-device production suite passes background intake with a held provider across reload, concurrent manual edits, retry, cancellation and adult review. Existing offline/isolation/original-photo/export checks still pass.

Release checks passed locally: 65 Rust tests, Clippy with warnings denied, Rust formatting, TypeScript, 29 frontend tests, 12 Python companion checks, Prettier and the frontend/PWA build. The complete production browser suite passed and the updated intake, listing and wardrobe screenshots were inspected. Publishing and live rollout completed as recorded below.

The photo-suggestion repair is deployed through homelab `ecc33b5`, using source `fe05d38` and immutable digest `sha256:9fd22a232fe14ddd2cfae76307400fe6fc2dd4c7393eb2d86a88b122fc4d9f08` from successful CI `38425275235`. Flux is Ready/Healthy, the expected image is running and the retained NFS claim is Bound. Private HTTPRoute conditions are Accepted/ResolvedRefs; the public gateway returns 404. The consistent pre-upgrade backup is `/data/pre-photo-fix-20261013.sqlite3`. Ports source `fe0d6cc` was committed/pushed; the successful checked rollout is recorded below.

Ports rollout completed through homelab `90f05e4`, using application source `fe0d6cc517fd4b6f325a88114fb6c4b5f239ac8b` and immutable digest `sha256:cda9dc974658e673ef51589ed608118411ccf95c6bd11e0c37399c770491f647` from successful CI `38428197988`. CI passed all checks and the expanded two-device browser suite against both the production build and packaged container. Flux is Ready/Healthy with the exact digest running, one ready replica, a Bound retained claim and the private-only HTTPRoute Accepted/ResolvedRefs. Fresh 390px and 1440px trusted HTTPS contexts verified the new background-photo bundle, active PWA, no page errors/overflow, denied anonymous state/photo/events/photo-job access and denied cross-origin photo-job writes. The public gateway still returns 404. Native production model discovery/chat passed after both rollouts.

The consistent pre-upgrade backup is `/data/pre-photo-ports-20261013.sqlite3`; the post-upgrade verification backup is `/data/post-photo-ports-20261013.sqlite3`. Schema 7, SQLite integrity and DELETE journaling passed. Existing inventory, child/location configuration, photos and accounts were retained, and the encrypted provider configuration/secret checksum is identical. Production verification created no clothing or sale records. Temporary diagnostic database copies were removed. All port and rollout milestones are complete; checked future updates to this existing private instance remain authorized.

## Quick add and batch intake — 2026-10-13

- [x] Rename background intake to Quick add and open a modal for multiple original photos with selectable front/back/tag/detail types.
- [x] Carry active child/location/status/category/size/season filters into explicit starting values, validate household ownership and preserve selected values through AI completion.
- [x] Submit without waiting for AI; display pending spinners and AI review markers on the inventory entries across devices and reloads.
- [ ] Validate multi-photo intake, filter retention, retry/idempotency and two-device production behavior; publish and privately deploy the checked release.

One Quick add creates one clothing entry from up to four photos. Reopening it retains the current filters for rapid intake of a child's clothes or a storage box. Text search is not a garment fact. Explicit filter values are kept while AI fills the remaining descriptive fields; children and locations stay household-scoped and are never sent to the model.

Quick add implementation checks passed: 66 Rust tests, Clippy with warnings denied, Rust formatting, TypeScript, 29 frontend tests, 12 companion checks, Prettier and the frontend/PWA build. The expanded two-device production browser suite passed front+tag intake with all concrete filters selected and the AI provider held pending. It verified immediate submission/close, card spinners/review markers, reopening an unsubmitted photo draft, starting another piece with the same filters, reload, concurrent edits, guarded duplicate requests, retry, cancellation and review. Selected category/size/season values survive model output; household-owned child/location IDs are checked transactionally. Schema 8 stores the immutable accepted request independently of retried-job baselines. Real mobile modal/pending-list screenshots were inspected. Publishing and private rollout are pending.
