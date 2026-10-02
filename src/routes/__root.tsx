import {
  createRootRoute,
  HeadContent,
  Outlet,
  Scripts,
} from '@tanstack/react-router'
import stylesheet from '../styles.css?url'
import wardrobeStyles from '../wardrobe.css?url'
import { AppProvider } from '../lib/state'
import { APP_NAME } from '../lib/brand'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      {
        name: 'viewport',
        content: 'width=device-width, initial-scale=1, viewport-fit=cover',
      },
      { title: `${APP_NAME} · Room to grow.` },
      { name: 'theme-color', content: '#244f8f' },
      {
        name: 'description',
        content: 'A shared family wardrobe, from first wear to the next home.',
      },
    ],
    links: [
      { rel: 'stylesheet', href: stylesheet },
      { rel: 'stylesheet', href: wardrobeStyles },
      { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
      { rel: 'manifest', href: '/manifest.webmanifest' },
      { rel: 'apple-touch-icon', href: '/icon-192.png' },
    ],
  }),
  component: () => (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        <AppProvider>
          <Outlet />
        </AppProvider>
        <Scripts />
      </body>
    </html>
  ),
})
