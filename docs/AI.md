# Configure photo and listing assistance

The app supports manual details and sale drafts until a provider is enabled. AI proposes changes for review; it does not save inventory or publish listings.

## Compatible API

In Settings, choose API transport and set a provider base URL, API key and vision-capable chat model. The server calls `/chat/completions` with private inline image data, validates the response and keeps credentials encrypted. Compatibility options cover strict JSON schema, JSON object or prompted JSON, token parameter and reasoning effort.

The provider must accept images to identify clothing/tag details. Text-only models can draft sale listings. Model catalog and connection tests use the configured provider. Connectivity does not establish accuracy on your clothes: check suggestions against the real garment and tag, especially brand, size and wear.

For background edits, choose an image model supporting `/images/edits` with inline reference images and a base64 result. Providers differ; unsupported editing reports an error and retains originals. The preservation prompt cannot guarantee unchanged garment details. Compare manually. Enhanced attachments are tracked on the server and excluded from Vinted exports.

## Personal Hermes companion

Choose companion transport, create a pairing token and copy it to the trusted agent machine. Use the masked setup prompt in `scripts/hermes-companion.py`, then run the worker in the same personal Hermes Python environment that contains its dependencies. The companion stores private configuration as `littlewardrobe-companion.json` under the personal Hermes home. Pair through its masked prompt, then start the worker:

```sh
python scripts/hermes-companion.py --setup https://littlewardrobe.fruitiex.org
python scripts/hermes-companion.py
```

It uses the locally owned personal login; no browser cookies, OAuth tokens or provider credentials are copied to the app. Pairing replacement/revocation invalidates old worker access. The agent machine must reach the private HTTPS origin through LAN/VPN.

Chat jobs preserve inline images and requested model/reasoning/response format. Image edits are translated into a Hermes provider call with the original attached as a reference. Choose a supported Hermes image model. If the companion is offline or its installed provider lacks the capability, the app reports the limitation and preserves the draft. Tests use controlled protocol fixtures; a live personal subscription is external configuration.

## Data sent and costs

Photo extraction sends up to four selected original clothing/tag images and intake notes to the chosen provider. The app shows evidence, uncertainty and a tag-photo request when size is unreadable. Suggestions do not establish authenticity or new condition.

Sale generation sends selected garment facts and optional recorded purchase price, excluding private notes, child names, birthdays and storage data. It requests Finnish text and a transparent starting price estimate. No comparable-market data is fetched. Background editing sends the original image and preservation instruction. Provider/subscription quotas apply to these explicit actions; generation is not automatic.
