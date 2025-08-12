interface Props {
  onPick: (files: FileList) => void;
}

export default function FilePicker({ onPick }: Props) {
  return (
    <input
      type="file"
      multiple
      onChange={(e) => e.target.files && onPick(e.target.files)}
    />
  );
}
