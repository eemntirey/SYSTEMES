import React, { useState } from 'react'

export const CalculatorApp: React.FC = () => {
  const [display, setDisplay] = useState('0')
  const [equation, setEquation] = useState('')
  const [waitingForOperand, setWaitingForOperand] = useState(false)
  const [prevValue, setPrevValue] = useState<number | null>(null)
  const [operation, setOperation] = useState<string | null>(null)

  const inputDigit = (digit: string) => {
    if (waitingForOperand) {
      setDisplay(digit)
      setWaitingForOperand(false)
    } else {
      setDisplay(display === '0' ? digit : display + digit)
    }
  }

  const inputDecimal = () => {
    if (waitingForOperand) {
      setDisplay('0.')
      setWaitingForOperand(false)
      return
    }
    if (!display.includes('.')) {
      setDisplay(display + '.')
    }
  }

  const clearAll = () => {
    setDisplay('0')
    setEquation('')
    setPrevValue(null)
    setOperation(null)
    setWaitingForOperand(false)
  }

  const clearEntry = () => {
    setDisplay('0')
  }

  const handleBackspace = () => {
    if (waitingForOperand) return
    if (display.length === 1 || (display.length === 2 && display.startsWith('-'))) {
      setDisplay('0')
    } else {
      setDisplay(display.slice(0, -1))
    }
  }

  const toggleSign = () => {
    const val = parseFloat(display)
    if (val !== 0) {
      setDisplay(String(-val))
    }
  }

  const performMathOperation = (nextOp: string) => {
    const inputValue = parseFloat(display)

    if (prevValue === null) {
      setPrevValue(inputValue)
      setEquation(`${inputValue} ${nextOp}`)
    } else if (operation) {
      const current = prevValue || 0
      let result = current

      switch (operation) {
        case '+':
          result = current + inputValue
          break
        case '−':
          result = current - inputValue
          break
        case '×':
          result = current * inputValue
          break
        case '÷':
          result = inputValue !== 0 ? current / inputValue : Number.NaN
          break
      }

      if (!Number.isFinite(result)) {
        setDisplay('Erreur')
        setPrevValue(null)
        setOperation(null)
        setWaitingForOperand(true)
        setEquation('Division par zéro impossible')
        return
      }
      setPrevValue(result)
      setDisplay(String(result))
      setEquation(`${result} ${nextOp}`)
    }

    setWaitingForOperand(true)
    setOperation(nextOp)
  }

  const handleEquals = () => {
    const inputValue = parseFloat(display)

    if (operation && prevValue !== null) {
      const current = prevValue
      let result = current

      switch (operation) {
        case '+':
          result = current + inputValue
          break
        case '−':
          result = current - inputValue
          break
        case '×':
          result = current * inputValue
          break
        case '÷':
          result = inputValue !== 0 ? current / inputValue : Number.NaN
          break
      }

      if (!Number.isFinite(result)) {
        setEquation('Division par zéro impossible')
        setDisplay('Erreur')
        setPrevValue(null)
        setOperation(null)
        setWaitingForOperand(true)
        return
      }
      setEquation(`${current} ${operation} ${inputValue} =`)
      setDisplay(String(result))
      setPrevValue(null)
      setOperation(null)
      setWaitingForOperand(true)
    }
  }

  // Quick single-operand functions
  const performFunction = (func: 'sqrt' | 'sqr' | 'inv' | 'percent') => {
    const val = parseFloat(display)
    let res = val
    if (func === 'sqrt') res = Math.sqrt(val)
    if (func === 'sqr') res = val * val
    if (func === 'inv') res = val !== 0 ? 1 / val : 0
    if (func === 'percent') res = val / 100
    setDisplay(Number.isFinite(res) ? String(res) : 'Erreur')
    setEquation(Number.isFinite(res) ? equation : 'Opération impossible')
    setWaitingForOperand(true)
  }

  const btnStyle = (isAccent = false, isOp = false): React.CSSProperties => ({
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: '5px',
    border: 'none',
    fontSize: '15px',
    fontWeight: isAccent ? 600 : isOp ? 500 : 400,
    backgroundColor: isAccent
      ? 'var(--win-accent)'
      : isOp
      ? 'rgba(255, 255, 255, 0.05)'
      : 'rgba(255, 255, 255, 0.08)',
    color: isAccent ? '#ffffff' : 'var(--text-primary)',
    cursor: 'pointer',
    transition: 'background 0.12s',
  })

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        backgroundColor: 'var(--window-bg)',
        padding: '12px',
      }}
    >
      {/* Top Header */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '8px',
        }}
      >
        <span style={{ fontSize: '13px', fontWeight: 600 }}>Standard</span>
      </div>

      {/* Display Screen */}
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'flex-end',
          justifyContent: 'flex-end',
          padding: '12px 6px',
          minHeight: '85px',
        }}
      >
        <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', minHeight: '16px' }}>
          {equation}
        </div>
        <div
          style={{
            fontSize: '38px',
            fontWeight: 600,
            color: 'var(--text-primary)',
            letterSpacing: '-0.5px',
          }}
        >
          {display}
        </div>
      </div>

      {/* Buttons Grid */}
      <div
        style={{
          flex: 1,
          display: 'grid',
          gridTemplateColumns: 'repeat(4, 1fr)',
          gridTemplateRows: 'repeat(6, 1fr)',
          gap: '3px',
        }}
      >
        {/* Row 1 */}
        <button onClick={() => performFunction('percent')} style={btnStyle(false, true)}>
          %
        </button>
        <button onClick={clearEntry} style={btnStyle(false, true)}>
          CE
        </button>
        <button onClick={clearAll} style={btnStyle(false, true)}>
          C
        </button>
        <button onClick={handleBackspace} style={btnStyle(false, true)}>
          ⌫
        </button>

        {/* Row 2 */}
        <button onClick={() => performFunction('inv')} style={btnStyle(false, true)}>
          ¹/x
        </button>
        <button onClick={() => performFunction('sqr')} style={btnStyle(false, true)}>
          x²
        </button>
        <button onClick={() => performFunction('sqrt')} style={btnStyle(false, true)}>
          √x
        </button>
        <button onClick={() => performMathOperation('÷')} style={btnStyle(false, true)}>
          ÷
        </button>

        {/* Row 3 */}
        <button onClick={() => inputDigit('7')} style={btnStyle()}>
          7
        </button>
        <button onClick={() => inputDigit('8')} style={btnStyle()}>
          8
        </button>
        <button onClick={() => inputDigit('9')} style={btnStyle()}>
          9
        </button>
        <button onClick={() => performMathOperation('×')} style={btnStyle(false, true)}>
          ×
        </button>

        {/* Row 4 */}
        <button onClick={() => inputDigit('4')} style={btnStyle()}>
          4
        </button>
        <button onClick={() => inputDigit('5')} style={btnStyle()}>
          5
        </button>
        <button onClick={() => inputDigit('6')} style={btnStyle()}>
          6
        </button>
        <button onClick={() => performMathOperation('−')} style={btnStyle(false, true)}>
          −
        </button>

        {/* Row 5 */}
        <button onClick={() => inputDigit('1')} style={btnStyle()}>
          1
        </button>
        <button onClick={() => inputDigit('2')} style={btnStyle()}>
          2
        </button>
        <button onClick={() => inputDigit('3')} style={btnStyle()}>
          3
        </button>
        <button onClick={() => performMathOperation('+')} style={btnStyle(false, true)}>
          +
        </button>

        {/* Row 6 */}
        <button onClick={toggleSign} style={btnStyle()}>
          +/-
        </button>
        <button onClick={() => inputDigit('0')} style={btnStyle()}>
          0
        </button>
        <button onClick={inputDecimal} style={btnStyle()}>
          ,
        </button>
        <button onClick={handleEquals} style={btnStyle(true)}>
          =
        </button>
      </div>
    </div>
  )
}
