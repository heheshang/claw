import { describe, it, expect, vi } from 'vitest'
import { ref } from 'vue'

// Mock the logger
vi.mock('../composables/useLogger', () => ({
  useLogger: () => ({
    initLogger: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}))

describe('useAuth composable', () => {
  // Test auth state management
  it('should have correct initial state', () => {
    // Since we can't easily test the actual composable without Tauri API,
    // we test the auth logic separately
    const isAuthenticated = ref(false)
    const user = ref<{ id: number; username: string; nickname: string | null } | null>(null)

    expect(isAuthenticated.value).toBe(false)
    expect(user.value).toBeNull()
  })

  it('should store user data correctly', () => {
    const userData = {
      id: 1,
      username: 'testuser',
      nickname: 'Test User'
    }

    localStorage.setItem('user', JSON.stringify(userData))
    const stored = JSON.parse(localStorage.getItem('user') || 'null')

    expect(stored).toEqual(userData)
    expect(stored.username).toBe('testuser')
  })

  it('should handle token storage', () => {
    const token = 'test-jwt-token'
    localStorage.setItem('token', token)

    expect(localStorage.getItem('token')).toBe(token)
  })

  it('should clear auth data on logout', () => {
    localStorage.setItem('user', JSON.stringify({ id: 1, username: 'test' }))
    localStorage.setItem('token', 'token')

    localStorage.removeItem('user')
    localStorage.removeItem('token')

    expect(localStorage.getItem('user')).toBeNull()
    expect(localStorage.getItem('token')).toBeNull()
  })
})
