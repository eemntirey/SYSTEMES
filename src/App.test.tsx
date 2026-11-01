import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import App from './App'
import { OSProvider } from './context/OSContext'
import { CalculatorApp } from './components/apps/CalculatorApp'
import { PhotosApp } from './components/apps/PhotosApp'
import { TerminalApp } from './components/apps/TerminalApp'
import { RecycleBinApp } from './components/apps/RecycleBinApp'
import { useOS } from './context/OSContext'

describe('Windows 11 Recreation Tests', () => {
  it('renders Desktop with essential shortcuts', () => {
    render(<App />)
    expect(screen.getAllByText('Ce PC').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Corbeille').length).toBeGreaterThan(0)
    expect(screen.getByText('Photos')).toBeDefined()
    expect(screen.queryByText('Microsoft Edge')).toBeNull()
    expect(screen.getByText('Bloc-notes')).toBeDefined()
    expect(screen.getByText('Terminal')).toBeDefined()
    expect(screen.getByText('Calculatrice')).toBeDefined()
  })

  it('toggles StartMenu when clicking Windows Start button', () => {
    render(<App />)
    const startBtn = screen.getByTitle('Démarrer')
    expect(startBtn).toBeDefined()

    // Click to open Start menu
    fireEvent.click(startBtn)
    expect(screen.getByPlaceholderText(/Rechercher des applications/i)).toBeDefined()
    expect(screen.getByText('Applications épinglées')).toBeDefined()

    // Click again to close
    fireEvent.click(startBtn)
    expect(screen.queryByText('Applications épinglées')).toBeNull()
  })

  it('reports division by zero instead of returning a fabricated result', () => {
    render(<OSProvider><CalculatorApp /></OSProvider>)
    fireEvent.click(screen.getByText('8'))
    fireEvent.click(screen.getByText('÷'))
    fireEvent.click(screen.getByText('0'))
    fireEvent.click(screen.getByText('='))
    expect(screen.getByText('Erreur')).toBeDefined()
    expect(screen.getByText('Division par zéro impossible')).toBeDefined()
  })

  it('performs calculations in CalculatorApp', () => {
    render(
      <OSProvider>
        <CalculatorApp />
      </OSProvider>
    )

    const sevenBtn = screen.getByText('7')
    const plusBtn = screen.getByText('+')
    const fiveBtn = screen.getByText('5')
    const equalsBtn = screen.getByText('=')

    fireEvent.click(sevenBtn)
    fireEvent.click(plusBtn)
    fireEvent.click(fiveBtn)
    fireEvent.click(equalsBtn)

    // 7 + 5 = 12
    expect(screen.getByText('12')).toBeDefined()
  })
})


describe('Recycle Bin', () => {
  const RecycleBinHarness = () => {
    const { deleteItem } = useOS()
    return (
      <>
        <button onClick={() => deleteItem('doc-welcome')}>Supprimer le document de test</button>
        <RecycleBinApp />
      </>
    )
  }

  it('moves a deleted file to the bin and restores it', () => {
    render(<OSProvider><RecycleBinHarness /></OSProvider>)
    fireEvent.click(screen.getByText('Supprimer le document de test'))
    expect(screen.getByText('Bienvenue sur Windows 11.txt')).toBeDefined()
    fireEvent.click(screen.getByTitle('Restaurer'))
    expect(screen.getByText('La Corbeille est vide')).toBeDefined()
  })
})


describe('Photos and Terminal', () => {
  it('navigates between gallery images', () => {
    render(<OSProvider><PhotosApp /></OSProvider>)
    const imagesBefore = screen.getAllByRole('img')
    const firstSource = imagesBefore[0].getAttribute('src')
    fireEvent.click(screen.getByLabelText('Photo suivante'))
    const imagesAfter = screen.getAllByRole('img')
    expect(imagesAfter[0].getAttribute('src')).not.toBe(firstSource)
  })

  it('executes pwd inside the simulated terminal', () => {
    render(<OSProvider><TerminalApp /></OSProvider>)
    const input = screen.getByRole('textbox')
    fireEvent.change(input, { target: { value: 'pwd' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(screen.getByText('C:\\Users\\eemntirey')).toBeDefined()
  })
})
