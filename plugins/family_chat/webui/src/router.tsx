import { createBrowserRouter, Navigate } from 'react-router-dom';
import Bootstrap from './pages/Bootstrap';
import Login from './pages/Login';
import Chat from './pages/Chat';
import Settings from './pages/Settings';
import Search from './pages/Search';

const router = createBrowserRouter([
  { path: '/bootstrap', element: <Bootstrap /> },
  { path: '/login', element: <Login /> },
  { path: '/', element: <Navigate to="/room/1" replace /> },
  { path: '/room/:id', element: <Chat /> },
  { path: '/dm/:userId', element: <Chat /> },
  { path: '/search', element: <Search /> },
  { path: '/settings', element: <Settings /> },
]);

export default router;
