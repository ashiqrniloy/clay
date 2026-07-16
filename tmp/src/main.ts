interface User {                                                                                                   
       name: string;                                                                                                  
       age: number;                                                                                                   
   }                                                                                                                  
                                                                                                                      
function greet(user: User): string {                                                                               
    return `Hello, ${user.name}! You are ${user.age} years old.`;                                                  
}                                                                                                                  
                                                                                                                      
const alice: User = { name: "Alice", age: 30 };                                                                    
console.log(greet(alice));
