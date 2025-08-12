import RoomList from './RoomList';
import DMList from './DMList';
import SearchBar from './SearchBar';

export default function Sidebar() {
  return (
    <aside className="w-60 border-r bg-gray-50 p-2 hidden md:block">
      <SearchBar />
      <RoomList />
      <DMList />
    </aside>
  );
}
