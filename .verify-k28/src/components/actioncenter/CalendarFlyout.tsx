import React, { useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const CalendarFlyout: React.FC = () => {
  const {
    isCalendarOpen,
    notifications,
    clearNotifications,
    removeNotification,
  } = useOS()

  const [currentDate, setCurrentDate] = useState(new Date())

  if (!isCalendarOpen) return null

  const year = currentDate.getFullYear()
  const month = currentDate.getMonth()

  const monthNames = [
    'Janvier', 'Février', 'Mars', 'Avril', 'Mai', 'Juin',
    'Juillet', 'Août', 'Septembre', 'Octobre', 'Novembre', 'Décembre'
  ]

  const daysOfWeek = ['Lu', 'Ma', 'Me', 'Je', 'Ve', 'Sa', 'Di']

  // Calculate calendar days
  const firstDayOfMonth = new Date(year, month, 1)
  let startingDay = firstDayOfMonth.getDay() - 1 // 0 = Monday
  if (startingDay === -1) startingDay = 6

  const daysInMonth = new Date(year, month + 1, 0).getDate()
  const today = new Date()

  const prevMonth = () => {
    setCurrentDate(new Date(year, month - 1, 1))
  }

  const nextMonth = () => {
    setCurrentDate(new Date(year, month + 1, 1))
  }

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        bottom: '56px',
        right: '12px',
        width: '360px',
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '12px',
        boxShadow: '0 20px 48px rgba(0,0,0,0.5)',
        zIndex: 99999,
        padding: '16px',
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        maxHeight: '680px',
        animation: 'slideRightFade 0.16s ease-out',
      }}
    >
      {/* Notifications Header */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          borderBottom: '1px solid var(--window-border)',
          paddingBottom: '8px',
        }}
      >
        <span style={{ fontSize: '13px', fontWeight: 600 }}>Notifications</span>
        {notifications.length > 0 && (
          <button
            onClick={clearNotifications}
            style={{
              border: 'none',
              background: 'transparent',
              color: 'var(--win-accent)',
              fontSize: '11.5px',
              cursor: 'pointer',
            }}
          >
            Effacer tout
          </button>
        )}
      </div>

      {/* Notifications List */}
      <div
        style={{
          maxHeight: '160px',
          overflowY: 'auto',
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
        }}
      >
        {notifications.length > 0 ? (
          notifications.map(n => (
            <div
              key={n.id}
              style={{
                padding: '10px 12px',
                borderRadius: '8px',
                backgroundColor: 'var(--flyout-card)',
                border: '1px solid var(--window-border)',
                position: 'relative',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
                <span style={{ fontSize: '11px', color: 'var(--text-tertiary)', fontWeight: 600 }}>
                  {n.appName} • {n.time}
                </span>
                <button
                  onClick={() => removeNotification(n.id)}
                  style={{
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-tertiary)',
                    cursor: 'pointer',
                    padding: '2px',
                  }}
                >
                  <FluentIcon name="x" size={11} />
                </button>
              </div>
              <div style={{ fontSize: '12.5px', fontWeight: 600, color: 'var(--text-primary)' }}>
                {n.title}
              </div>
              <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '2px' }}>
                {n.message}
              </div>
            </div>
          ))
        ) : (
          <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', fontSize: '12px', padding: '16px' }}>
            Aucune nouvelle notification
          </div>
        )}
      </div>

      {/* Calendar Header with Navigation */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          paddingTop: '8px',
          borderTop: '1px solid var(--window-border)',
        }}
      >
        <span style={{ fontSize: '14px', fontWeight: 600 }}>
          {monthNames[month]} {year}
        </span>
        <div style={{ display: 'flex', gap: '4px' }}>
          <button
            onClick={prevMonth}
            style={{
              border: 'none',
              background: 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              padding: '4px',
              borderRadius: '4px',
            }}
          >
            <FluentIcon name="chevron-up" size={14} />
          </button>
          <button
            onClick={nextMonth}
            style={{
              border: 'none',
              background: 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              padding: '4px',
              borderRadius: '4px',
            }}
          >
            <FluentIcon name="chevron-down" size={14} />
          </button>
        </div>
      </div>

      {/* Days of Week Header */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(7, 1fr)',
          textAlign: 'center',
          fontSize: '11.5px',
          color: 'var(--text-tertiary)',
          fontWeight: 600,
        }}
      >
        {daysOfWeek.map(d => (
          <div key={d} style={{ padding: '4px' }}>
            {d}
          </div>
        ))}
      </div>

      {/* Calendar Month Grid */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(7, 1fr)',
          gap: '2px',
          textAlign: 'center',
        }}
      >
        {/* Padding days before month starts */}
        {Array.from({ length: startingDay }).map((_, i) => (
          <div key={`empty-${i}`} style={{ height: '32px' }} />
        ))}

        {/* Days in current month */}
        {Array.from({ length: daysInMonth }).map((_, i) => {
          const dayNumber = i + 1
          const isToday =
            dayNumber === today.getDate() &&
            month === today.getMonth() &&
            year === today.getFullYear()

          return (
            <div
              key={dayNumber}
              style={{
                height: '32px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                borderRadius: '50%',
                fontSize: '12px',
                cursor: 'pointer',
                backgroundColor: isToday ? 'var(--win-accent)' : 'transparent',
                color: isToday ? '#ffffff' : 'var(--text-primary)',
                fontWeight: isToday ? 600 : 400,
              }}
              onMouseEnter={e => {
                if (!isToday) e.currentTarget.style.backgroundColor = 'var(--control-hover)'
              }}
              onMouseLeave={e => {
                if (!isToday) e.currentTarget.style.backgroundColor = 'transparent'
              }}
            >
              {dayNumber}
            </div>
          )
        })}
      </div>
    </div>
  )
}
