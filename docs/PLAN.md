# Little Wardrobe product and implementation plan

Build a working family application in `/home/rasse/clothes` to manage the children’s clothes from purchase through daily use, storage, hand-me-downs and resale. The user requested a complete application inspired by `~/forkbasket`, with AI assistance for photo intake and sale preparation. This document preserves the agreed scope, practical constraints, implementation decisions and progress so work can continue across sessions.

## Agreed requirements

- Build a working app, not only a product proposal.
- Family-only, mobile-friendly web app, installable as a PWA.
- English interface and Finnish sale listings.
- Follow ForkBasket’s self-hosting architecture and configurable AI options, including an OpenAI-compatible endpoint and the personal Hermes companion.
- Full scope: inventory, photo intake, configurable children, active rotation, storage, hand-me-downs, sale drafts, bundles and posting automation where feasible.
- Prepare a private ingress so the application is accessible only through the family’s LAN or VPN. Do not add a public gateway. Deployment preparation is authorized; changing the live cluster requires an explicit deployment instruction.
- Save this plan and make its completion the active goal, as requested by the user.

The working name is **Little Wardrobe**. It is an implementation default, not a user-approved brand or a claim of name availability. Real child names, birthdays, sizes and inventory start empty and are configured through the app.

## Photo intake and AI review

The primary flow is take or choose a clothing photo, optionally attach a separate tag photo, request AI suggestions, review uncertain details, assign a child and storage location, then save. Support front, back, tag and detail photographs on the same inventory entry. Keep original photographs distinguishable from optional edited images.

AI suggests descriptive name, clothing category, visible brand, printed size and size system, color, material where readable, likely season and visible condition. Show field evidence and uncertainty. An unreadable size triggers a request for a tag photo or manual entry; do not infer size from garment proportions or child age. Condition from a photograph is provisional. Keep visible defects and allow the family to confirm wear, stains, pilling and repairs. Purchase price, purchase date and purchase source remain optional manual facts unless evidence is explicitly supplied. Do not invent provenance, authenticity, waterproof performance or home conditions.

Support manual intake when AI is disconnected or unavailable. Errors must preserve the draft and uploaded images. Photo upload and AI calls require connectivity; saved inventory edits use ForkBasket’s persisted offline outbox. Initial support is JPEG, PNG and WebP, with clear limits; HEIC is a possible later enhancement rather than a promised first-release capability.

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

Use TanStack Start/Router, React, TypeScript, accessible Radix dialogs and the ForkBasket visual conventions, adapted for a calm clothing inventory. Use Rust, Tokio, Axum, SQLite and generated TypeScript API types. Start from the committed ForkBasket foundation without changing that project or copying its live data, keys, credentials or uncommitted work.

Reuse authenticated household isolation, first-owner setup token, account recovery, expiring invitations, secure same-origin cookie sessions, encrypted AI credentials, private uploaded images, atomic operations, idempotency, guarded concurrent edits, SSE refresh and persisted offline changes. Keep API keys and companion credentials on the appropriate server/agent machine. Test clothing-specific input validation, household image ownership and malformed AI output. Do not assume that previously tested shopping behavior proves the clothing features work.

Use a project-local locked Nix environment, npm and Cargo lockfiles. Package frontend and backend into a single unprivileged container with persistent data and health checks. Provide a private Gateway API HTTPRoute using the homelab’s private gateway, HTTPS for PWA/camera behavior, a single replica, suitable SQLite storage, and SOPS secret examples. Document installation, provider setup, invitation, backups, recovery and upgrade procedures. Do not deploy the live homelab during implementation without a deployment instruction.

Commit and push completed, checked task changes as authorized by `/home/rasse/AGENTS.md`. A new private repository may be created if needed to fulfill that standing push instruction. Do not include unrelated ForkBasket changes.

## Completion checklist

- [x] Inspect ForkBasket and capture the user’s preferences and scope.
- [x] Save this plan and activate completion as the goal.
- [x] Bootstrap the isolated application from committed ForkBasket infrastructure and verify tools in Nix.
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

The working application now includes the English mobile wardrobe, configurable children and storage, private photo/tag intake, evidence and uncertainty review, size checks, hand-me-downs, Finnish single-item and bundle drafts, approval, guarded exports, published links and actual sale history. The family starts empty. The shared/offline foundation was copied from committed ForkBasket source without changing ForkBasket or copying live data or credentials.

Production browser checks have passed with two adult accounts and independent devices. They cover disjoint edits, rejected same-field conflicts, private photos, unreadable size followed by a tag photo, reference-image editing, offline reload/reconnect, original-only Vinted export, approval resets, sale tracking, bundle preparation, JSON backup/restore and 320–1440px layouts. Actual screenshots are saved in `mockups/` and have been visually reviewed. Browser AI fixtures and illustrated garments are isolated test data.

Delivery checks passed: 60 Rust tests, Clippy for all targets with warnings denied, Rust formatting, 28 frontend tests, TypeScript, five Python companion protocol tests, Prettier, the frontend/PWA build and private Kubernetes template rendering. Clothing tests cover private inline photos, reviewed drafts, offline companion errors, malformed output and edited-photo provenance. The final Nix container passed the complete production browser suite as UID 10001 with a read-only root and isolated temporary data. The image is available locally as `ghcr.io/fruitiex/littlewardrobe:main` and through the `result-container` build artifact.

The completed application is delivered in the private repository [FruitieX/littlewardrobe](https://github.com/FruitieX/littlewardrobe). The validation/publishing workflow builds and verifies the container before publishing to GHCR; it has no live deployment job. All accepted completion milestones are implemented and checked. External provider configuration, live deployment and any future business-only publisher are documented operational steps outside this delivery.

Primary marketplace documentation establishes business-only integration routes, not an available personal-account publisher. The accepted fallback is implemented and documented in `docs/MARKETPLACES.md`. Provider credentials and a live personal companion remain external configuration. Private deployment and recovery instructions are in `docs/DEPLOYMENT.md`; templates do not alter the live cluster. No marketplace publication or live deployment has occurred.
