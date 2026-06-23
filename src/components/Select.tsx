import ReactSelect, { type StylesConfig } from 'react-select';
export interface SelectOption { value: string; label: string; }
interface Props {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
}
const styles: StylesConfig<SelectOption, false> = {
  control: (b, s) => ({ ...b, minHeight: 34, backgroundColor: 'white', borderColor: s.isFocused ? '#eab308' : '#e7d27a', boxShadow: 'none', ':hover': { borderColor: '#eab308' } }),
  option: (b, s) => ({ ...b, fontSize: 14, backgroundColor: s.isSelected ? '#fde047' : s.isFocused ? '#fef3c7' : 'white', color: '#1c1917' }),
  singleValue: (b) => ({ ...b, color: '#1c1917' }),
  menu: (b) => ({ ...b, zIndex: 20 }),
  indicatorSeparator: () => ({ display: 'none' }),
};
export default function Select({ value, options, onChange }: Props) {
  return (
    <ReactSelect
      className="text-sm min-w-[12rem]"
      options={options}
      value={options.find(o => o.value === value) ?? null}
      onChange={(opt) => opt && onChange(opt.value)}
      isSearchable
      styles={styles}
    />
  );
}
