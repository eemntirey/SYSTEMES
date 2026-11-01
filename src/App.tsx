import React from 'react'
import { OSProvider, useOS } from './context/OSContext'
import { Desktop } from './components/desktop/Desktop'
import { Window } from './components/window/Window'
import { Taskbar } from './components/taskbar/Taskbar'
import { StartMenu } from './components/startmenu/StartMenu'
import { SearchFlyout } from './components/search/SearchFlyout'
import { QuickSettings } from './components/actioncenter/QuickSettings'
import { CalendarFlyout } from './components/actioncenter/CalendarFlyout'
import { WidgetsBoard } from './components/widgets/WidgetsBoard'
import { LockScreen } from './components/lockscreen/LockScreen'
import { MenuBar } from './components/menu/MenuBar'

// App components
import { FileExplorerApp } from './components/apps/FileExplorerApp'
import { NotepadApp } from './components/apps/NotepadApp'
import { PhotosApp } from './components/apps/PhotosApp'
import { RecycleBinApp } from './components/apps/RecycleBinApp'
import { TerminalApp } from './components/apps/TerminalApp'
import { CalculatorApp } from './components/apps/CalculatorApp'
import { SettingsApp } from './components/apps/SettingsApp'
import { ProcessManagerApp } from './components/apps/ProcessManagerApp'
import { PaintApp } from './components/apps/PaintApp'
import { WindowState } from './types'

const WindowsDesktopContent: React.FC = () => {
  const { windows } = useOS()

  const renderAppContent = (win: WindowState) => {
    switch (win.appId) {
      case 'explorer':
        return (
          <FileExplorerApp
            initialPath={win.customData?.initialPath as string | undefined}
          />
        )
      case 'notepad':
        return (
          <NotepadApp
            fileId={win.customData?.fileId as string | undefined}
            filePath={win.customData?.filePath as string | undefined}
            fileContent={win.customData?.fileContent as string | undefined}
          />
        )
      case 'photos':
        return <PhotosApp />
      case 'recycle':
        return <RecycleBinApp />
      case 'terminal':
        return <TerminalApp />
      case 'calculator':
        return <CalculatorApp />
      case 'settings':
        return <SettingsApp />
      case 'taskmanager':
        return <ProcessManagerApp />
      case 'paint':
        return <PaintApp />
      default:
        return (
          <div style={{ padding: '20px', color: 'var(--text-secondary)' }}>
            Application non trouvée
          </div>
        )
    }
  }

  return (
    <div style={{ position: 'absolute', top: 0, left: 0, width: '100vw', height: '100vh', overflow: 'hidden', margin: 0, padding: 0 }}>
      {/* macOS-style Menu Bar */}
      <MenuBar />

      {/* Desktop & Window Container */}
      <Desktop>
        {windows.map(win => (
          <Window key={win.id} window={win}>
            {renderAppContent(win)}
          </Window>
        ))}
      </Desktop>

      {/* Flyouts & Modals */}
      <StartMenu />
      <SearchFlyout />
      <QuickSettings />
      <CalendarFlyout />
      <WidgetsBoard />

      {/* Centered Taskbar */}
      <Taskbar />

      {/* Lock / Sleep Screen */}
      <LockScreen />
    </div>
  )
}

export const App: React.FC = () => {
  return (
    <OSProvider>
      <WindowsDesktopContent />
    </OSProvider>
  )
}

export default App
