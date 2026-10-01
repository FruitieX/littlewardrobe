import { describe, expect, it } from 'vitest'
import { initialState, recipes } from './data'
import type { AppState } from './data'
import { discovery, recipeScore, suggestWeek } from './learning'
const state = (): AppState => ({
  ...initialState(),
  items: [],
  plan: {},
  favoriteRecipeIds: [],
  preferences: {
    timeZone: 'UTC',
    servings: 4,
    dietary: [],
    avoid: [],
    novelty: 0.3,
    maxWeekdayMinutes: 30,
    autoRestock: false,
  },
})
describe('household meal suggestions', () => {
  it('honors avoided ingredients and excludes them from schedules', () => {
    const household = state()
    household.preferences!.avoid = ['pasta']
    expect(recipeScore(recipes[0], household).score).toBeLessThan(-999)
    expect(suggestWeek(recipes, household).map((r) => r.id)).not.toContain(
      'green-pasta',
    )
  })
  it('uses favorites, actual cooking feedback and pantry overlap as evidence', () => {
    const household = state()
    household.favoriteRecipeIds = ['green-pasta']
    household.mealFeedback = {
      '2026-09-29': {
        recipeId: 'green-pasta',
        status: 'cooked',
        servings: 4,
        at: '2026-09-29T12:00:00Z',
        rating: 5,
        consumed: true,
      },
    }
    const score = recipeScore(recipes[0], household, Date.parse('2026-09-30'))
    expect(score.reasons).toContain('A family favorite')
    expect(score.reasons).toContain('Cooked recently')
    expect(score.reasons).toContain('Family rating 5.0/5')
  })
  it('keeps discovery a draft and respects dismissal', () => {
    const household = state()
    const suggestion = discovery(recipes, household)!
    expect(recipes.some((r) => r.id === suggestion.id)).toBe(false)
    household.suggestionFeedback = {
      [suggestion.id]: {
        status: 'dismissed',
        at: new Date().toISOString(),
        until: null,
      },
    }
    expect(discovery(recipes, household)).toBeNull()
  })
})
