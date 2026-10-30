# Configure photo and listing assistance

The app supports manual details and sale drafts until a provider is enabled. AI proposes details for review and never publishes listings. Explicit background photo intake saves a marked clothing draft that an adult must review.

## Compatible API

In Settings, choose API transport and set a provider base URL, API key and vision-capable chat model. The server calls `/chat/completions` with private inline image data, validates the response and keeps credentials encrypted. Compatibility options cover strict JSON schema, JSON object or prompted JSON, token parameter and reasoning effort.

The provider must accept images to identify clothing/tag details. Text-only models can draft sale listings. Model catalog and connection tests use the configured provider. Connectivity does not establish accuracy on your clothes: check suggestions against the real garment and tag, especially brand, size and wear.

For background edits, choose an image model supporting `/images/edits` with inline reference images and a base64 result. Providers differ; unsupported editing reports an error and retains originals. The preservation prompt cannot guarantee unchanged garment details. Compare manually. Enhanced attachments are tracked on the server and excluded from Vinted exports.

## Quick add in the background

Use **Quick add** in the wardrobe to open a modal for one piece. Add up to four front, back, tag or detail photos, using the camera or gallery, then choose each photo's type. The active child, location, status, category, size and season filters become editable starting values; search text is not copied. Explicit starting values remain unchanged when AI finishes the other fields. Unselected category, size, size system and season can be suggested by AI.

Press **Add to wardrobe** once the uploads finish. The modal closes after the entry and task are saved, without waiting for AI. The entry appears under the matching filters with a loading spinner and **AI · needs review**. Reopen Quick add to add the next piece with the same filters. Closing an unsubmitted modal keeps its uploaded photos while you remain on that wardrobe screen; a failed upload can be retried or discarded. An uncertain submission keeps the same request for a duplicate-safe retry.

Once saved, you can put the phone away. Reading continues on the server through closed/reloaded pages and server restarts. The existing vision model, reasoning and personal/API connection are reused.

Pending tasks show **Reading photo…** with **Cancel AI**. Failed tasks keep the original photo and expose **Retry AI**, **Dismiss**, and the item editor for manual entry. Cancellation prevents any late result from attaching; an already-started provider call may still complete. Interrupted calls may repeat after a server restart. Repeating an accepted queue request does not create another item.

Completed entries show **AI · needs review**. Open one to inspect evidence and uncertain fields, add a readable tag photo if needed, choose the child/location and save. Saving records your review and cancels unfinished work. An AI result only fills descriptive fields that have not changed since it was queued. It never selects a child/location, infers an unreadable size or overwrites manual changes. Deletion, replaced photos, review or cancellation prevent late results from reviving an item. Sale approval remains blocked until the clothing has been reviewed and its size/condition confirmed. Vinted exports keep using original photos.

Up to 30 unfinished photo tasks may be queued per household. Intake and retry are limited to 60 requests per hour per running server; photo requests run one at a time. Household JSON restore preserves the entries/photos and review markers but cancels outstanding tasks. Full SQLite backups also retain the durable queue. Schema 8 retains each accepted Quick add request and its selected values across retries; take a pre-upgrade backup for downgrade recovery.

## Personal Hermes companion

Choose companion transport, create a pairing token and copy it to the trusted agent machine. Use the masked setup prompt in `scripts/hermes-companion.py`, then run the worker in the same personal Hermes Python environment that contains its dependencies. The companion stores private configuration as `littlewardrobe-companion.json` under the personal Hermes home. Pair through its masked prompt, then start the worker:

```sh
python scripts/hermes-companion.py --setup https://littlewardrobe.example.com
python scripts/hermes-companion.py
```

It uses the locally owned personal login; no browser cookies, OAuth tokens or provider credentials are copied to the app. Pairing replacement/revocation invalidates old worker access. The agent machine must be able to reach the app's private HTTPS origin.

Use a dedicated profile with one locally owned OAuth credential labeled `personal`. Create it with `hermes profile create littlewardrobe --no-alias`, then sign in with `hermes -p littlewardrobe auth add openai-codex --type oauth --label personal`. Copying a rotating OAuth grant between profiles is unsafe. Run `nix develop --command node scripts/hermes-companion.mjs --profile littlewardrobe --setup https://littlewardrobe.example.com` to pair, then the same command without `--setup` to start the worker.

The worker refreshes only that owned grant under the profile-local auth-store lock. It does not invoke the general CLI credential pool, borrow root logins or access Codex CLI authentication. For persistent operation, run it as a sandboxed user service (for example `littlewardrobe-companion.service` declared through NixOS/Home Manager).

Chat jobs preserve inline images and requested model/reasoning/response format. Image edits are translated into a Hermes provider call with the original attached as a reference. Choose a supported Hermes image model. If the companion is offline or its installed provider lacks the capability, the app reports the limitation and preserves the draft. Tests use controlled protocol fixtures; a live personal subscription is external configuration.

## Data sent and costs

Photo extraction sends up to four selected original clothing/tag images and intake notes to the chosen provider. The app shows evidence, uncertainty and a tag-photo request when size is unreadable. Suggestions do not establish authenticity or new condition.

Sale generation sends selected garment facts and optional recorded purchase price, excluding private notes, child names, birthdays and storage data. It requests Finnish text and a transparent starting price estimate. No comparable-market data is fetched. Background editing sends the original image and preservation instruction. Provider/subscription quotas apply to these explicit actions; generation is not automatic.
