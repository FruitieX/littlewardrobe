import * as Dialog from '@radix-ui/react-dialog'
import { X, ArrowUpRight } from 'lucide-react'
import type { ReactNode } from 'react'
import { WardrobeMark } from './WardrobeIdentity'
import { APP_NAME } from '../lib/brand'
import { FoodIcon } from './FoodIcon'

export function Brand() {
  return (
    <span className="brand">
      <span className="brand-mark">
        <WardrobeMark size={32} />
      </span>
      <span>{APP_NAME}</span>
      <span className="brand-dot">.</span>
    </span>
  )
}
export function Modal({
  open,
  onOpenChange,
  title,
  description,
  children,
  className = '',
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  children: ReactNode
  className?: string
}) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="modal-overlay" />
        <Dialog.Content className={`modal-content ${className}`}>
          <div className="modal-header">
            <Dialog.Close
              className="icon-button modal-close"
              aria-label="Close dialog"
            >
              <X size={20} />
            </Dialog.Close>
            <Dialog.Title className="modal-title">{title}</Dialog.Title>
            <Dialog.Description className="modal-description">
              {description}
            </Dialog.Description>
          </div>
          <div className="modal-body">{children}</div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
export function Eyebrow({ children }: { children: ReactNode }) {
  return <span className="eyebrow">{children}</span>
}
export function SectionTitle({
  title,
  count,
  children,
}: {
  title: string
  count?: number
  children?: ReactNode
}) {
  return (
    <div className="section-heading">
      <h2>
        {title} {count !== undefined && <span className="count">{count}</span>}
      </h2>
      {children}
    </div>
  )
}
export function EmptyState({
  icon,
  title,
  text,
  children,
}: {
  icon: string
  title: string
  text: string
  children?: ReactNode
}) {
  return (
    <div className="empty-state">
      <span className="empty-icon">
        <FoodIcon value={icon} />
      </span>
      <h3>{title}</h3>
      <p>{text}</p>
      {children}
    </div>
  )
}
export function SmallArrow() {
  return <ArrowUpRight size={17} />
}
