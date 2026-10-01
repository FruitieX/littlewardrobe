import { describe, it, expect } from 'vitest'
import {
  blankGarment,
  fits,
  manualListing,
  resetApprovals,
  recordChanges,
} from './wardrobe'
import { diffState, optimistic } from './sync'
import { initialState } from './data'

describe('family clothing operations', () => {
  it('compares printed numeric sizes only within their size systems', () => {
    const child = {
      id: 'a',
      name: 'A',
      birthDate: '',
      clothingSize: '104',
      shoeSize: '27',
      color: 'sage',
    }
    expect(fits({ ...blankGarment(), size: '98/104' }, child)).toBe('now')
    expect(fits({ ...blankGarment(), size: '98' }, child)).toBe('small')
    expect(fits({ ...blankGarment(), size: '110' }, child)).toBe('later')
    expect(
      fits({ ...blankGarment(), size: '2–3 years', sizeSystem: 'age' }, child),
    ).toBe('unknown')
    expect(
      fits({ ...blankGarment(), size: '26', sizeSystem: 'eu-shoe' }, child),
    ).toBe('small')
  })
  it('saves only edited fields against the values shown when a form opened', () => {
    const original = { ...blankGarment(), name: 'Original', notes: 'Private' }
    const changes = recordChanges('garments', original, {
      ...original,
      name: 'My edit',
    })
    expect(changes).toEqual([
      {
        collection: 'garments',
        key: original.id,
        field: 'name',
        before: 'Original',
        after: 'My edit',
      },
    ])
    const concurrent = {
      ...initialState(),
      garments: [{ ...original, notes: 'Other device note' }],
    }
    const result = optimistic(concurrent, {
      type: 'patch',
      label: 'Edited name',
      changes,
    })
    expect(result.garments?.[0].name).toBe('My edit')
    expect(result.garments?.[0].notes).toBe('Other device note')
    expect(recordChanges('garments', null, original)[0].before).toBe(null)
  })
  it('makes Finnish bundles and preserves originals while excluding enhanced photos', () => {
    const a = {
      ...blankGarment(),
      name: 'A',
      size: '98',
      condition: 'good',
      defects: 'Jälki hihassa',
      photos: [
        { url: '/api/images/a', kind: 'front' },
        { url: '/api/images/b', kind: 'enhanced' },
      ],
    }
    const b = { ...blankGarment(), name: 'B', size: '104' }
    const draft = manualListing([a, b], 'vinted')
    expect(draft.title).toContain('paketti')
    expect(draft.description).toContain('Jälki hihassa')
    expect(draft.photos).toEqual(['/api/images/a'])
    expect(draft.price).toBe(0)
    expect(draft.status).toBe('draft')
  })
  it('queues clothing field changes and invalidates approvals after a size correction', () => {
    const garment = {
      ...blankGarment(),
      id: 'a',
      name: 'Shirt',
      size: '98',
      status: 'sell',
    }
    const listing = { ...manualListing([garment], 'tori'), status: 'approved' }
    const changed = { ...garment, size: '104' }
    const before = {
      ...initialState(),
      garments: [garment],
      children: [],
      locations: [],
      saleListings: [listing],
    }
    const after = {
      ...before,
      garments: [changed],
      saleListings: resetApprovals([listing], garment, changed),
    }
    const changes = diffState(before, after)
    expect(
      changes.some((c) => c.collection === 'garments' && c.field === 'size'),
    ).toBe(true)
    expect(
      changes.some(
        (c) =>
          c.collection === 'saleListings' &&
          c.field === 'status' &&
          c.after === 'draft',
      ),
    ).toBe(true)
    const applied = optimistic(before, {
      type: 'patch',
      label: 'Size correction',
      changes,
    })
    expect(applied.garments?.[0].size).toBe('104')
    expect(applied.saleListings?.[0].status).toBe('draft')
  })
})
