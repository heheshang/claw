import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock Tauri
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}))

describe('useLogger composable', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('should create logger instance', async () => {
    const { useLogger } = await import('../composables/useLogger')
    const logger = useLogger()

    expect(logger).toBeDefined()
    expect(typeof logger.info).toBe('function')
    expect(typeof logger.warn).toBe('function')
    expect(typeof logger.error).toBe('function')
    expect(typeof logger.initLogger).toBe('function')
  })

  it('should initialize without errors', async () => {
    const { useLogger } = await import('../composables/useLogger')
    const { initLogger } = useLogger()

    await expect(initLogger()).resolves.not.toThrow()
  })

  it('should log info messages', async () => {
    const { useLogger } = await import('../composables/useLogger')
    const { info, initLogger } = useLogger()

    await initLogger()
    expect(() => info('test message')).not.toThrow()
  })

  it('should log warn messages', async () => {
    const { useLogger } = await import('../composables/useLogger')
    const { warn, initLogger } = useLogger()

    await initLogger()
    expect(() => warn('warning message')).not.toThrow()
  })

  it('should log error messages', async () => {
    const { useLogger } = await import('../composables/useLogger')
    const { error, initLogger } = useLogger()

    await initLogger()
    expect(() => error('error message')).not.toThrow()
  })
})
