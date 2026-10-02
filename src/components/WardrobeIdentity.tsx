export function WardrobeMark({ size = 32 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="none"
      aria-hidden="true"
    >
      <rect width="64" height="64" rx="18" fill="#244f8f" />
      <path
        d="M28 15c0-6 9-6 9 0 0 3-5 4-5 8L14 33h36L32 23"
        stroke="#ffd0bd"
        strokeWidth="3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="m24 30-11 8 6 8 6-4v13h14V42l6 4 6-8-11-8c-4 5-12 5-16 0Z"
        fill="#f5f7fb"
      />
      <path
        d="M29 48h6"
        stroke="#f08f71"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  )
}
export function WardrobeRail() {
  return (
    <svg
      className="wardrobe-rail"
      viewBox="0 0 340 240"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M22 35h296"
        stroke="#244f8f"
        strokeWidth="5"
        strokeLinecap="round"
      />
      <circle cx="22" cy="35" r="6" fill="#244f8f" />
      <circle cx="318" cy="35" r="6" fill="#244f8f" />
      <path
        d="M96 42c0-12 18-12 18 0 0 8-10 8-10 16L52 85h104L104 58M230 42c0-12 18-12 18 0 0 8-10 8-10 16l-52 27h104l-52-27"
        stroke="#244f8f"
        strokeWidth="3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="m83 79-42 31 22 33 23-15v77h40v-77l23 15 22-33-42-31c-11 18-35 18-46 0Z"
        fill="#f08f71"
        stroke="#cb6b53"
        strokeWidth="2"
        strokeLinejoin="round"
      />
      <path
        d="M88 190h34M49 113l14 21M157 113l-14 21"
        stroke="#ffe5d8"
        strokeWidth="2"
        strokeDasharray="3 4"
      />
      <path
        d="m216 79-38 29 20 29 20-13v86h45v-86l19 13 21-29-38-29c-12 18-38 18-49 0Z"
        fill="#6587bd"
        stroke="#244f8f"
        strokeWidth="2"
        strokeLinejoin="round"
      />
      <path
        d="M219 99h42M218 114h45M218 130h45M218 146h45M218 162h45M218 178h45M221 196h39"
        stroke="#b9cdee"
        strokeWidth="3"
      />
      <rect
        x="250"
        y="171"
        width="29"
        height="38"
        rx="4"
        fill="#fffaf5"
        stroke="#244f8f"
        strokeWidth="1.5"
        transform="rotate(10 250 171)"
      />
      <circle cx="262" cy="180" r="2" fill="#f08f71" />
      <path
        d="m258 191 12 2m-13 4 9 1"
        stroke="#244f8f"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  )
}
