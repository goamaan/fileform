export type FileFamily='table'|'image'|'pdf'|'audio'|'video';
export type TaskID='table.convert'|'image.convert'|'pdf.combine'|'pdf.split'|'pdf.images'|'pdf.extract-images'|'pdf.compress'|'text.extract'|'text.ocr'|'audio.convert'|'audio.trim'|'video.convert'|'video.trim';
export interface TaskDefinition {id:TaskID;label:string;families:FileFamily[];multiple?:boolean;outputFormats:string[]}
export const tasks:TaskDefinition[]=[
 {id:'table.convert',label:'Convert table',families:['table'],outputFormats:['json','csv','tsv']},
 {id:'image.convert',label:'Convert or resize image',families:['image'],outputFormats:['png','jpeg','tiff']},
 {id:'pdf.combine',label:'Combine or organize pages',families:['pdf','image'],multiple:true,outputFormats:['pdf']},
 {id:'pdf.split',label:'Split PDF',families:['pdf'],outputFormats:['pdf']},
 {id:'pdf.images',label:'Export page images',families:['pdf','image'],multiple:true,outputFormats:['png','jpeg']},
 {id:'pdf.extract-images',label:'Extract embedded images',families:['pdf'],multiple:true,outputFormats:['images']},
 {id:'pdf.compress',label:'Compress PDF',families:['pdf'],outputFormats:['pdf']},
 {id:'text.extract',label:'Extract text',families:['pdf'],outputFormats:['txt']},
 {id:'text.ocr',label:'Recognize text',families:['image','pdf'],outputFormats:['txt']},
 {id:'audio.convert',label:'Convert or extract audio',families:['audio','video'],outputFormats:['wav','flac','m4a','mp3']},
 {id:'audio.trim',label:'Trim audio',families:['audio','video'],outputFormats:['wav','flac','m4a']},
 {id:'video.convert',label:'Convert or resize video',families:['video'],outputFormats:['mp4','mov']},
 {id:'video.trim',label:'Trim video',families:['video'],outputFormats:['mp4','mov']},
];
export function availableTasks(families:FileFamily[]):TaskDefinition[]{
 return tasks.filter(task=>families.length>0&&(families.length===1||task.multiple)&&families.every(family=>task.families.includes(family)));
}
