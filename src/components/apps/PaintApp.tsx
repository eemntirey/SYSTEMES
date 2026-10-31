import React, { useRef, useState, useEffect } from 'react'
import { FluentIcon } from '../common/FluentIcon'

export const PaintApp: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [isDrawing, setIsDrawing] = useState(false)
  const [tool, setTool] = useState<'pen' | 'eraser'>('pen')
  const [color, setColor] = useState('#0078d4')
  const [brushSize, setBrushSize] = useState(4)
  const undoStack = useRef<string[]>([])
  const redoStack = useRef<string[]>([])

  const saveHistory = () => {
    const canvas = canvasRef.current
    if (!canvas) return
    undoStack.current.push(canvas.toDataURL())
    if (undoStack.current.length > 30) undoStack.current.shift()
    redoStack.current = []
  }

  const restoreSnapshot = (snapshot: string) => {
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return
    const image = new Image()
    image.onload = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      ctx.drawImage(image, 0, 0, canvas.width, canvas.height)
    }
    image.src = snapshot
  }

  const undo = () => {
    const canvas = canvasRef.current
    if (!canvas || !undoStack.current.length) return
    redoStack.current.push(canvas.toDataURL())
    restoreSnapshot(undoStack.current.pop() as string)
  }

  const redo = () => {
    const canvas = canvasRef.current
    if (!canvas || !redoStack.current.length) return
    undoStack.current.push(canvas.toDataURL())
    restoreSnapshot(redoStack.current.pop() as string)
  }

  const PALETTE = [
    '#000000',
    '#7f7f7f',
    '#ffffff',
    '#880015',
    '#ed1c24',
    '#ff7f27',
    '#fff200',
    '#22b14c',
    '#00a2e8',
    '#3f48cc',
    '#a349a4',
    '#0078d4',
  ]

  // Setup canvas background
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
  }, [])

  const startDrawing = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const rect = canvas.getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top

    saveHistory()
    ctx.beginPath()
    ctx.moveTo(x, y)
    setIsDrawing(true)
  }

  const draw = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawing) return
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const rect = canvas.getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top

    ctx.strokeStyle = tool === 'eraser' ? '#ffffff' : color
    ctx.lineWidth = tool === 'eraser' ? brushSize * 3 : brushSize
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    ctx.lineTo(x, y)
    ctx.stroke()
  }

  const stopDrawing = () => {
    setIsDrawing(false)
  }

  const clearCanvas = () => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    saveHistory()
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
  }

  const downloadCanvas = () => {
    const canvas = canvasRef.current
    if (!canvas) return
    const link = document.createElement('a')
    link.download = 'mon-dessin-paint.png'
    link.href = canvas.toDataURL()
    link.click()
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', backgroundColor: 'var(--window-bg)' }}>
      {/* Ribbon Toolbar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '16px',
          padding: '8px 16px',
          borderBottom: '1px solid var(--window-border)',
          backgroundColor: 'rgba(255, 255, 255, 0.02)',
          flexWrap: 'wrap',
        }}
      >
        {/* Tools: Pen / Eraser */}
        <div style={{ display: 'flex', gap: '4px' }}>
          <button
            onClick={() => setTool('pen')}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '6px 10px',
              borderRadius: '4px',
              border: tool === 'pen' ? '1px solid var(--win-accent)' : '1px solid transparent',
              backgroundColor: tool === 'pen' ? 'var(--win-accent-light)' : 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              fontSize: '12px',
            }}
          >
            <FluentIcon name="palette" size={14} color="var(--win-accent)" />
            <span>Pinceau</span>
          </button>

          <button
            onClick={() => setTool('eraser')}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '6px 10px',
              borderRadius: '4px',
              border: tool === 'eraser' ? '1px solid var(--win-accent)' : '1px solid transparent',
              backgroundColor: tool === 'eraser' ? 'var(--win-accent-light)' : 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              fontSize: '12px',
            }}
          >
            <FluentIcon name="trash" size={14} />
            <span>Gomme</span>
          </button>
        </div>

        <div style={{ width: '1px', height: '24px', backgroundColor: 'var(--window-border)' }} />

        {/* Brush Size */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <span>Taille :</span>
          <input
            type="range"
            min="1"
            max="24"
            value={brushSize}
            onChange={e => setBrushSize(Number(e.target.value))}
            style={{ width: '80px', accentColor: 'var(--win-accent)' }}
          />
          <span style={{ minWidth: '18px' }}>{brushSize}px</span>
        </div>

        <div style={{ width: '1px', height: '24px', backgroundColor: 'var(--window-border)' }} />

        {/* Color Palette */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          {PALETTE.map(c => {
            const isSelected = tool === 'pen' && color.toLowerCase() === c.toLowerCase()
            return (
              <button
                key={c}
                onClick={() => {
                  setColor(c)
                  setTool('pen')
                }}
                style={{
                  width: '20px',
                  height: '20px',
                  borderRadius: '3px',
                  backgroundColor: c,
                  border: isSelected ? '2px solid var(--win-accent)' : '1px solid rgba(0,0,0,0.3)',
                  boxShadow: isSelected ? '0 0 6px var(--win-accent)' : 'none',
                  cursor: 'pointer',
                }}
              />
            )
          })}
        </div>

        <div style={{ width: '1px', height: '24px', backgroundColor: 'var(--window-border)' }} />

        {/* Actions */}
        <div style={{ display: 'flex', gap: '6px', marginLeft: 'auto' }}>
          <button onClick={undo} disabled={!undoStack.current.length} style={{ padding: '5px 10px', borderRadius: '4px', border: '1px solid var(--window-border)', background: 'transparent', color: 'var(--text-primary)', fontSize: '12px', cursor: undoStack.current.length ? 'pointer' : 'not-allowed', opacity: undoStack.current.length ? 1 : .45 }}>Annuler</button>
          <button onClick={redo} disabled={!redoStack.current.length} style={{ padding: '5px 10px', borderRadius: '4px', border: '1px solid var(--window-border)', background: 'transparent', color: 'var(--text-primary)', fontSize: '12px', cursor: redoStack.current.length ? 'pointer' : 'not-allowed', opacity: redoStack.current.length ? 1 : .45 }}>Rétablir</button>
          <button
            onClick={clearCanvas}
            style={{
              padding: '5px 12px',
              borderRadius: '4px',
              border: '1px solid var(--window-border)',
              backgroundColor: 'transparent',
              color: 'var(--text-primary)',
              fontSize: '12px',
              cursor: 'pointer',
            }}
          >
            Effacer tout
          </button>
          <button
            onClick={downloadCanvas}
            style={{
              padding: '5px 12px',
              borderRadius: '4px',
              border: 'none',
              backgroundColor: 'var(--win-accent)',
              color: '#ffffff',
              fontSize: '12px',
              fontWeight: 500,
              cursor: 'pointer',
            }}
          >
            Télécharger PNG
          </button>
        </div>
      </div>

      {/* Drawing Canvas Area */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#1f242d',
          padding: '20px',
        }}
      >
        <canvas
          ref={canvasRef}
          width={760}
          height={480}
          onMouseDown={startDrawing}
          onMouseMove={draw}
          onMouseUp={stopDrawing}
          onMouseLeave={stopDrawing}
          style={{
            backgroundColor: '#ffffff',
            borderRadius: '4px',
            boxShadow: '0 8px 30px rgba(0,0,0,0.5)',
            cursor: tool === 'pen' ? 'crosshair' : 'default',
          }}
        />
      </div>
    </div>
  )
}
