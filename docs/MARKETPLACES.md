# Marketplace posting and photo support

Little Wardrobe prepares reviewed Finnish listings and exports their text and selected photographs. Publication happens in the marketplace. Approval inside Little Wardrobe does not publish anything, and the app does not collect Tori or Vinted passwords or browser sessions.

## Posting feasibility

The official [Vinted Pro Integrations documentation](https://pro-docs.svc.vinted.com/#access), checked on 2026-10-01, offers item creation, updates, deletion, order data and webhooks, but access is restricted to an allowlisted set of Vinted Pro businesses. A normal family account cannot use that documented route. The documentation includes a sandbox, signed requests and catalogue mappings that would support a future approved integration if eligible access is obtained.

[Tori’s official Tori-kauppa page](https://www.tori.fi/yrityksille/tori-kauppa/Tori-kauppa-kokeile-ilmaiseksi), checked on the same date, advertises API/feed integrations for its business product. The reviewed personal-account help and terms did not establish a supported personal publishing API. This is a feasibility limit of the interfaces verified for this project, not proof that Tori has no internal APIs.

The family application therefore ships with copy, download, browser file sharing where supported, marketplace links, manual publication confirmation and external listing URLs. Direct posting remains unavailable for the family’s ordinary accounts. No accounts were contacted, upgraded or registered with a marketplace during implementation.

If supported access becomes available, integrate it as a separate publisher that reads only the latest approved draft, verifies item availability and photo provenance, maps required platform categories, saves a durable idempotent publication request, and records the marketplace result. A timeout must trigger reconciliation before retrying. Confirm publication from the platform before moving a listing to `listed`. Use polling if callbacks cannot reach the private LAN/VPN ingress. Cancellation and deletion need distinct review; selling an item elsewhere must flag related listings for removal.

## Photos and factual accuracy

The [Finnish Vinted catalogue rules](https://www.vinted.fi/catalog-rules), reviewed on 2026-10-01, require seller-taken photographs, prohibit editing, and require defects to be disclosed. Vinted export excludes enhanced photographs. The backend stores provenance and rejects relabelling an edited image as an original; it also checks that referenced photos belong to the household and selected items.

Original uploads are decoded, oriented, stripped of metadata and resized for private storage. Their garment content is preserved. This normalization is distinct from the optional background-edit capability. Enhanced images remain separate attachments, require human comparison and are never included in Vinted exports. A provider can still alter garment details despite a preservation prompt, so the app does not assert that AI edits are faithful.

An item cannot be in two live Little Wardrobe listings on the same platform, including individual/bundle overlap. The application cannot detect listings created directly in a marketplace. Recording a sale flags overlapping tracked listings as withdrawn and asks the family to remove them externally. This does not delete remote listings automatically.

## Pricing

AI price suggestions use supplied clothing attributes and optional recorded purchase price. They are starting estimates, not verified market comparisons or evidence of actual sale prices. The user chooses the asking price and records the actual amount received. The app does not scrape listings for price estimates. Private notes, child names and birthdays are excluded from listing-generation context.
